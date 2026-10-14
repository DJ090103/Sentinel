import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

/**
 * The class strings behind every `Button` variant/size combination.
 *
 * Exported separately from the component because some elements need to *look*
 * like a button without *being* one — a router `<Link>`, for instance. Those
 * call `buttonVariants({ variant: "ghost" })` directly to borrow the styles.
 *
 * This file is vendored from shadcn/ui rather than imported from a package:
 * shadcn ships source, not a dependency, so component styles stay editable in
 * this repo. See product-docs/ARCHITECTURE.md, "Frontend styling".
 */
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-2xl text-sm font-bold transition-all active:translate-y-[3px] active:shadow-none disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-[0_4px_0_0_var(--primary-edge)] hover:brightness-105",
        destructive:
          "bg-destructive text-background shadow-[0_4px_0_0_color-mix(in_oklab,var(--destructive)_60%,black)] hover:brightness-105 focus-visible:ring-destructive/30",
        outline:
          "border-2 bg-transparent shadow-[0_3px_0_0_var(--border)] hover:bg-accent hover:text-accent-foreground",
        secondary: "bg-secondary text-secondary-foreground hover:bg-accent active:translate-y-0",
        ghost: "hover:bg-accent hover:text-accent-foreground active:translate-y-0",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-11 px-5 py-2 has-[>svg]:px-4",
        sm: "h-9 rounded-xl gap-1.5 px-4 has-[>svg]:px-3",
        lg: "h-14 rounded-2xl px-8 text-base has-[>svg]:px-6",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

/**
 * The standard button for the whole app.
 *
 * `asChild` renders the caller's own child element with the button's styling
 * instead of emitting a `<button>`. That matters for accessibility: a control
 * that navigates should be an `<a>`, not a `<button>` that calls `navigate()`,
 * and this prop is how it gets button styling without lying about its role.
 */
function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  }) {
  const Comp = asChild ? Slot : "button";

  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
