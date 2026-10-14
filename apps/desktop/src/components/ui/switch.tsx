import { cn } from "@/lib/utils";

/** An on/off switch with a visible label, usable by keyboard and screen reader. */
export function Switch({
  checked,
  onChange,
  label,
  description,
  disabled,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b py-3 last:border-b-0">
      <div className="min-w-0">
        <p className="font-bold">{label}</p>
        {description && <p className="text-muted-foreground text-sm">{description}</p>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cn(
          "focus-visible:ring-ring relative h-8 w-14 shrink-0 rounded-full border-2 outline-none transition-colors focus-visible:ring-[3px] disabled:opacity-50",
          checked ? "bg-primary border-primary" : "bg-muted",
        )}
      >
        <span
          className={cn(
            "bg-background absolute top-0.5 left-0.5 size-6 rounded-full transition-transform",
            checked && "translate-x-6",
          )}
        />
      </button>
    </div>
  );
}
