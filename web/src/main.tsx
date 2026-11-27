import { mountDashboard } from "@nat1an/mcp-backbone-dashboard"

import "./index.css"
import { spotifyLinkHealth } from "@/link-health"
import { Login } from "@/login"
import { LinkCard } from "@/overview/link-card"
import { NowPlayingCard } from "@/overview/now-playing-card"
import { ProfileCard } from "@/overview/profile-card"

mountDashboard({
  name: "Spotify",
  logo: "/spotify.png",
  logoClassName: "size-8",
  login: Login,
  loginDescription:
    "Let AI agents search, manage your playlists and library, and control playback on your Spotify account, with permissions you choose.",
  loginFooter:
    "Signing in also links your Spotify account. Your Spotify tokens are stored encrypted, and you can disconnect any agent at any time.",
  linkHealth: spotifyLinkHealth,
  overviewCards: [ProfileCard, NowPlayingCard, LinkCard],
  toolGroups: {
    users: "Profiles",
    search: "Search",
    catalog: "Catalog",
    library: "Library",
    personalization: "Top items & history",
    playlists: "Playlists",
    player: "Playback",
  },
})
