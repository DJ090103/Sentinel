/**
 * Title and one-line description at the top of a page.
 *
 * Exists so the routes don't each invent their own heading sizes and spacing —
 * the shell should look like one app. The display face is reserved for
 * headings and big numbers; everything else stays in the body face.
 */
export function PageHeader({
  title,
  description,
  readOnly = false,
}: {
  title: string;
  description: string;
  /** Adds the "nothing is removed until you confirm" badge on scanning pages. */
  readOnly?: boolean;
}) {
  return (
    <header className="space-y-2">
      <h1 className="font-display text-4xl font-extrabold tracking-tight">{title}</h1>
      <p className="text-muted-foreground max-w-prose text-base">{description}</p>
      {readOnly && (
        <p className="bg-success/15 text-success inline-flex items-center gap-2 rounded-full px-3 py-1 text-sm font-bold">
          Scanning only reads. Nothing is removed until you confirm.
        </p>
      )}
    </header>
  );
}
