import { Card, CardContent } from "@/components/ui/card";
import type { HomeAnalysis } from "@/lib/analysis";
import { formatBytes } from "@/lib/system-info";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Card className="gap-2 py-4">
      <CardContent className="space-y-2 px-4">
        <h3 className="text-sm font-semibold">{title}</h3>
        {children}
      </CardContent>
    </Card>
  );
}

function Row({ path, size }: { path: string; size: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4 text-sm">
      <span className="text-muted-foreground min-w-0 truncate" title={path}>
        {path}
      </span>
      <span className="shrink-0 tabular-nums">{size}</span>
    </div>
  );
}

/**
 * The home-folder report. Read-only by design: it shows where files are and
 * offers no delete, because these are the user's own files.
 */
export function AnalysisReport({ analysis }: { analysis: HomeAnalysis }) {
  return (
    <div className="space-y-4">
      <p className="text-muted-foreground text-sm">
        Looked at {analysis.totalFiles.toLocaleString()} files ({formatBytes(analysis.totalBytes)})
        in your home folder, skipping hidden folders and build output.
        {analysis.truncated && " Stopped early at the file limit, so totals are partial."}
      </p>

      <Section title="Biggest folders">
        {analysis.folders.map((f) => (
          <Row key={f.path} path={f.path} size={formatBytes(f.sizeBytes)} />
        ))}
      </Section>

      <Section title="Largest files">
        {analysis.largestFiles.map((f) => (
          <Row key={f.path} path={f.path} size={formatBytes(f.sizeBytes)} />
        ))}
      </Section>

      <Section title="Duplicate files">
        {analysis.duplicates.length === 0 && (
          <p className="text-muted-foreground text-sm">No identical large files found.</p>
        )}
        {analysis.duplicates.map((group) => (
          <div key={group.paths[0]} className="space-y-1">
            <p className="text-sm font-medium">
              {group.paths.length} copies of {formatBytes(group.sizeBytes)} —{" "}
              {formatBytes(group.sizeBytes * (group.paths.length - 1))} wasted
            </p>
            {group.paths.map((p) => (
              <Row key={p} path={p} size="" />
            ))}
          </div>
        ))}
      </Section>
    </div>
  );
}
