// Service worker of the dashboard PWA. It only handles Web Push: the server
// notifies about tool calls landing in the outbox, and clicking a notification
// opens the call's outbox page. Nothing is cached offline.

self.addEventListener("install", () => self.skipWaiting())
self.addEventListener("activate", (event) =>
  event.waitUntil(self.clients.claim())
)

self.addEventListener("push", (event) => {
  let data = {}
  try {
    data = event.data ? event.data.json() : {}
  } catch {
    data = { body: event.data ? event.data.text() : "" }
  }
  // Every push must show a notification: browsers (Safari in particular)
  // revoke subscriptions that receive silent pushes.
  event.waitUntil(
    self.registration.showNotification(data.title || "Telegram MCP", {
      body: data.body || "",
      tag: data.tag,
      icon: "/icon-192.png",
      badge: "/icon-192.png",
      data: { url: data.url || "/outbox" },
      requireInteraction: Boolean(data.expires_at),
    })
  )
})

self.addEventListener("notificationclick", (event) => {
  event.notification.close()
  const url = new URL(
    event.notification.data?.url || "/outbox",
    self.location.origin
  ).href
  event.waitUntil(openOrFocus(url))
})

async function openOrFocus(url) {
  const windows = await self.clients.matchAll({
    type: "window",
    includeUncontrolled: true,
  })
  const client = windows.find(
    (client) => new URL(client.url).origin === self.location.origin
  )
  if (client) {
    await client.focus()
    // The app routes client-side, keeping its state (see src/lib/push.ts).
    client.postMessage({ type: "navigate", url })
    return
  }
  await self.clients.openWindow(url)
}

// The browser rotated the subscription: hand the new one to the server.
self.addEventListener("pushsubscriptionchange", (event) => {
  event.waitUntil(resubscribe(event))
})

async function resubscribe(event) {
  const subscription =
    event.newSubscription ||
    (await self.registration.pushManager.subscribe(
      event.oldSubscription?.options ?? { userVisibleOnly: true }
    ))
  await fetch("/api/push/subscriptions", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(subscription.toJSON()),
  })
}
