import { CircleAlertIcon } from "lucide-react"
import { useSearchParams } from "react-router"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { withQuery } from "@/lib/api"

export function LoginPage() {
  const [searchParams] = useSearchParams()
  const next = searchParams.get("next") ?? "/"
  const notAllowed = searchParams.get("error") === "not_allowed"

  return (
    <div className="flex min-h-svh items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <img src="/telegram.png" alt="" className="size-8" />
            Telegram MCP
          </CardTitle>
          <CardDescription>
            Sign in to manage which AI agents can use your Telegram account.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {notAllowed && (
            <Alert variant="destructive">
              <CircleAlertIcon />
              <AlertTitle>Access denied</AlertTitle>
              <AlertDescription>
                This GitHub account isn't allowed to use this server.
              </AlertDescription>
            </Alert>
          )}
          {/* A full-page navigation: the server redirects to GitHub. */}
          <Button
            size="lg"
            render={<a href={withQuery("/auth/github", { next })} />}
            nativeButton={false}
          >
            Sign in with GitHub
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}
