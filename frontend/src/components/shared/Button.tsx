import * as React from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "outline";
export type ButtonSize    = "sm" | "md" | "lg" | "icon";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  isLoading?: boolean;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:   "bg-accent text-[#1a0e05] font-semibold hover:bg-accent-hover active:scale-[0.98] shadow-sm",
  secondary: "bg-elevated text-primary border border-border hover:bg-overlay hover:border-white/10 active:scale-[0.98]",
  ghost:     "text-secondary hover:text-primary hover:bg-elevated active:scale-[0.98]",
  outline:   "border border-border text-secondary hover:border-white/15 hover:text-primary active:scale-[0.98]",
  danger:    "bg-danger/15 text-danger border border-danger/30 hover:bg-danger/25 active:scale-[0.98]",
};

const sizeStyles: Record<ButtonSize, string> = {
  sm:   "h-7 px-3 text-xs rounded-md gap-1.5",
  md:   "h-9 px-4 text-sm rounded-lg gap-2",
  lg:   "h-11 px-6 text-base rounded-xl gap-2.5",
  icon: "h-9 w-9 rounded-lg",
};

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      variant = "secondary",
      size = "md",
      isLoading = false,
      leftIcon,
      rightIcon,
      className,
      children,
      disabled,
      ...props
    },
    ref,
  ) => {
    return (
      <button
        ref={ref}
        disabled={disabled || isLoading}
        className={cn(
          "inline-flex items-center justify-center font-medium transition-all duration-150 cursor-pointer",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/50",
          "disabled:opacity-50 disabled:cursor-not-allowed disabled:pointer-events-none",
          variantStyles[variant],
          sizeStyles[size],
          className,
        )}
        {...props}
      >
        {isLoading ? (
          <Loader2 size={14} className="animate-spin shrink-0" />
        ) : leftIcon ? (
          <span className="shrink-0">{leftIcon}</span>
        ) : null}
        {size !== "icon" && children && (
          <span className="truncate">{children}</span>
        )}
        {size === "icon" && !isLoading && children}
        {rightIcon && !isLoading && (
          <span className="shrink-0">{rightIcon}</span>
        )}
      </button>
    );
  },
);
Button.displayName = "Button";
