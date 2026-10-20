import {
  ActivityIcon,
  BotIcon,
  HouseIcon,
  PlugIcon,
  SendIcon,
  type LucideIcon,
} from "lucide-react"
import { matchPath, NavLink, useLocation } from "react-router"

import { NavUser } from "@/components/nav-user"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { useOutbox } from "@/hooks/use-tool-calls"

type NavItem = { to: string; label: string; icon: LucideIcon; end?: boolean }

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Overview", icon: HouseIcon, end: true },
  { to: "/sessions", label: "Sessions", icon: BotIcon },
  { to: "/outbox", label: "Outbox", icon: SendIcon },
  { to: "/activity", label: "Activity", icon: ActivityIcon },
  { to: "/connect", label: "Connect", icon: PlugIcon },
]

export function AppSidebar() {
  const { pathname } = useLocation()
  const pendingCount = useOutbox().data?.length ?? 0

  return (
    <Sidebar>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" render={<NavLink to="/" />}>
              <img src="/telegram.png" alt="" className="size-8" />
              <span className="font-semibold">Telegram MCP</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV_ITEMS.map(({ to, label, icon: Icon, end = false }) => (
                <SidebarMenuItem key={to}>
                  <SidebarMenuButton
                    isActive={matchPath({ path: to, end }, pathname) !== null}
                    render={<NavLink to={to} end={end} />}
                  >
                    <Icon />
                    <span>{label}</span>
                  </SidebarMenuButton>
                  {to === "/outbox" && pendingCount > 0 && (
                    <SidebarMenuBadge>{pendingCount}</SidebarMenuBadge>
                  )}
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <NavUser />
      </SidebarFooter>
    </Sidebar>
  )
}
