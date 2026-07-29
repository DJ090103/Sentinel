/**
 * Title and one-line description at the top of a page.
 *
 * Exists so the four routes don't each invent their own heading sizes and
 * spacing — the shell should look like one app, not four pages that happened to
 * be written on different days.
 */
export function PageHeader({ title, description }: { title: string; description: string }) {
  return (
    <header className="space-y-1">
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="text-muted-foreground text-sm">{description}</p>
    </header>
  );
}
