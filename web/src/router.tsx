import { createBrowserRouter, Navigate } from "react-router"

import { AppLayout } from "@/components/app-layout"
import { ActivityPage } from "@/pages/activity/activity-page"
import { OutboxItemPage } from "@/pages/outbox/outbox-item-page"
import { OutboxPage } from "@/pages/outbox/outbox-page"
import { ConnectPage } from "@/pages/connect/connect-page"
import { ConsentPage } from "@/pages/consent/consent-page"
import { LoginPage } from "@/pages/login-page"
import { OverviewPage } from "@/pages/overview/overview-page"
import { SessionDetailPage } from "@/pages/sessions/session-detail-page"
import { SessionsPage } from "@/pages/sessions/sessions-page"

export const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  { path: "/consent/:id", element: <ConsentPage /> },
  {
    element: <AppLayout />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "sessions", element: <SessionsPage /> },
      { path: "sessions/:id", element: <SessionDetailPage /> },
      { path: "outbox", element: <OutboxPage /> },
      // Wide enough for the editor, its preview and the links side by side.
      {
        path: "outbox/:id",
        element: <OutboxItemPage />,
        handle: { wide: true },
      },
      { path: "activity", element: <ActivityPage /> },
      { path: "connect", element: <ConnectPage /> },
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
])
