import { forwardRef, useState, useEffect } from "react";
import { cn } from "@hypr/ui/lib/utils";

interface AvatarProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: "sm" | "md" | "lg" | "xl";
  shape?: "circle" | "square";
}

export const Avatar = forwardRef<HTMLDivElement, AvatarProps>(
  ({ className, size = "md", shape = "circle", ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "relative flex shrink-0 overflow-hidden",
        // Size variants
        size === "sm" && "h-8 w-8",
        size === "md" && "h-10 w-10",
        size === "lg" && "h-12 w-12",
        size === "xl" && "h-14 w-14",
        // Shape variants
        shape === "circle" && "rounded-full",
        shape === "square" && "rounded-lg",
        className
      )}
      {...props}
    />
  )
);
Avatar.displayName = "Avatar";

interface AvatarImageProps extends React.ImgHTMLAttributes<HTMLImageElement> {
  /**
   * Optional alt text for the image.
   * If not provided, the image will be treated as decorative.
   */
  alt?: string;
  /**
   * Called when the image fails to load
   */
  onError?: () => void;
}

export const AvatarImage = forwardRef<HTMLImageElement, AvatarImageProps>(
  ({ className, alt, onError, ...props }, ref) => {
    const [hasError, setHasError] = useState(false);

    const handleError = () => {
      setHasError(true);
      onError?.();
    };

    // Reset error state if src changes
    useEffect(() => {
      setHasError(false);
    }, [props.src]);

    if (hasError) {
      return null;
    }

    return (
      <img
        ref={ref}
        className={cn(
          "aspect-square h-full w-full object-cover",
          className
        )}
        alt={alt}
        onError={handleError}
        {...props}
      />
    );
  }
);
AvatarImage.displayName = "AvatarImage";

interface AvatarFallbackProps extends React.HTMLAttributes<HTMLDivElement> {
  /**
   * Delay in milliseconds before showing the fallback element.
   * @default 600
   */
  delayMs?: number;
  /**
   * Background color variant for the fallback.
   * @default "neutral"
   */
  variant?: "neutral" | "primary" | "success" | "warning" | "danger";
}

export const AvatarFallback = forwardRef<HTMLDivElement, AvatarFallbackProps>(
  ({ className, variant = "neutral", delayMs = 600, ...props }, ref) => {
    const [isVisible, setIsVisible] = useState(false);

    useEffect(() => {
      const timer = setTimeout(() => setIsVisible(true), delayMs);
      return () => clearTimeout(timer);
    }, [delayMs]);

    if (!isVisible) {
      return null;
    }

    return (
      <div
        ref={ref}
        className={cn(
          "absolute inset-0 flex items-center justify-center text-sm font-medium",
          // Variant styles
          variant === "neutral" && "bg-neutral-100 text-neutral-600",
          variant === "primary" && "bg-blue-100 text-blue-600",
          variant === "success" && "bg-green-100 text-green-600",
          variant === "warning" && "bg-yellow-100 text-yellow-600",
          variant === "danger" && "bg-red-100 text-red-600",
          className
        )}
        {...props}
      />
    );
  }
);
AvatarFallback.displayName = "AvatarFallback";

export type { AvatarProps, AvatarImageProps, AvatarFallbackProps };