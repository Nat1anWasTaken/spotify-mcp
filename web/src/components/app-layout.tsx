import { cn } from "cn"
import { Outlet, useMatches } from "react-router"

import { AppSidebar } from "@/components/app-sidebar"
import { RequireAuth } from "@/components/require-auth"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"

/** Routes with `handle: { wide: true }` get more room than the default. */
function useWide() {
  return useMatches().some(
    (match) => (match.handle as { wide?: boolean } | undefined)?.wide
  )
}

export function AppLayout() {
  const wide = useWide()

  return (
    <RequireAuth>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <header className="flex h-12 items-center gap-2 px-4">
            <SidebarTrigger />
          </header>
          <div
            className={cn(
              "flex w-full flex-col gap-6 p-4 md:p-6",
              wide ? "max-w-7xl" : "max-w-5xl"
            )}
          >
            <Outlet />
          </div>
        </SidebarInset>
      </SidebarProvider>
    </RequireAuth>
  )
}
