import { Link } from "react-router";

import { PageHeader } from "@/components/page-header";
import { buttonVariants } from "@/components/ui/button";

/**
 * Rendered for any hash that doesn't match a route.
 *
 * Worth having even though a desktop window has no address bar to mistype into:
 * the hash survives across launches, so a route that is renamed or removed in a
 * later version strands anyone who quit while on it. Sitting inside the layout
 * means the sidebar is still there and the way out is one click.
 */
export function NotFoundPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="Page not found"
        description="That address doesn't match anything in Sentinel."
      />
      <Link to="/" className={buttonVariants({ variant: "secondary", size: "sm" })}>
        Back to Dashboard
      </Link>
    </div>
  );
}
