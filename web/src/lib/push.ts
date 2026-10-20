import { api } from "@/lib/api"
import { router } from "@/router"

/** How this browser can receive outbox notifications. */
export type PushSupport =
  | "supported"
  // iOS and iPadOS only deliver Web Push to apps added to the Home Screen.
  | "needs-install"
  | "unsupported"

export function pushSupport(): PushSupport {
  const capable =
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  if (capable) {
    return "supported"
  }
  return isAppleMobile() && !isStandalone() ? "needs-install" : "unsupported"
}

function isAppleMobile(): boolean {
  return (
    /iPad|iPhone|iPod/.test(navigator.userAgent) ||
    // iPadOS reports itself as a Mac.
    (navigator.userAgent.includes("Macintosh") && navigator.maxTouchPoints > 1)
  )
}

export function isStandalone(): boolean {
  return (
    window.matchMedia("(display-mode: standalone)").matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true
  )
}

/** Registers the service worker and follows its notification clicks. */
export function registerServiceWorker() {
  if (!("serviceWorker" in navigator)) {
    return
  }
  navigator.serviceWorker.register("/sw.js").catch((error: unknown) => {
    console.error("Service worker registration failed", error)
  })
  navigator.serviceWorker.addEventListener("message", (event) => {
    const data = event.data as { type?: string; url?: string } | null
    if (data?.type === "navigate" && data.url) {
      const url = new URL(data.url)
      void router.navigate(url.pathname + url.search)
    }
  })
}

export async function currentSubscription(): Promise<PushSubscription | null> {
  const registration = await navigator.serviceWorker.ready
  return registration.pushManager.getSubscription()
}

/** Asks for permission and subscribes this browser. Must run in a user
 * gesture (a click), or Safari refuses. */
export async function subscribe(): Promise<PushSubscription> {
  const permission = await Notification.requestPermission()
  if (permission !== "granted") {
    throw new Error(
      permission === "denied"
        ? "Notifications are blocked. Allow them in your browser's site settings."
        : "Notification permission wasn't granted."
    )
  }
  const { public_key } = await api.get<{ public_key: string }>("/api/push/key")
  const registration = await navigator.serviceWorker.ready
  let subscription = await registration.pushManager.getSubscription()
  // A subscription made for another server key can't receive our pushes.
  if (
    subscription &&
    !sameKey(subscription.options.applicationServerKey, public_key)
  ) {
    await subscription.unsubscribe()
    subscription = null
  }
  subscription ??= await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: base64UrlToBytes(public_key),
  })
  await api.post("/api/push/subscriptions", subscription.toJSON())
  return subscription
}

export async function unsubscribe(subscription: PushSubscription) {
  await api.delete("/api/push/subscriptions", {
    endpoint: subscription.endpoint,
  })
  await subscription.unsubscribe()
}

/** Closes the notification about a call once it is decided. */
export async function closeNotification(tag: string) {
  if (!("serviceWorker" in navigator)) {
    return
  }
  const registration = await navigator.serviceWorker.getRegistration()
  const notifications = (await registration?.getNotifications({ tag })) ?? []
  notifications.forEach((notification) => notification.close())
}

function sameKey(key: ArrayBuffer | null, base64Url: string): boolean {
  if (!key) {
    return false
  }
  const expected = base64UrlToBytes(base64Url)
  const actual = new Uint8Array(key)
  return (
    actual.length === expected.length &&
    actual.every((byte, i) => byte === expected[i])
  )
}

function base64UrlToBytes(value: string): Uint8Array<ArrayBuffer> {
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/")
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4)
  return Uint8Array.from(atob(padded), (char) => char.charCodeAt(0))
}
