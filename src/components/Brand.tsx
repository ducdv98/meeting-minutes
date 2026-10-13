import type { ComponentProps } from "react";
import darkWordmark from "../assets/wordmark-dark.png";
import whiteWordmark from "../assets/wordmark-white.png";
import darkMark from "../assets/mark-dark.png";
import whiteMark from "../assets/mark-white.png";
import { cn } from "../lib/cn";

function ThemeImage({ dark, white, className, ...props }: Omit<ComponentProps<"picture">, "children"> & { dark: string; white: string; alt: string }) {
  const { alt, ...pictureProps } = props;
  return <picture className={cn("inline-block", className)} {...pictureProps}>
    <source media="(prefers-color-scheme: dark)" srcSet={white} />
    <img src={dark} alt={alt} className="block h-full w-full object-contain object-left" />
  </picture>;
}
export function Wordmark({ className, ...props }: Omit<ComponentProps<"picture">, "children"> & { alt?: string }) {
  return <ThemeImage dark={darkWordmark} white={whiteWordmark} alt="Meeting Minutes" className={cn("h-8", className)} {...props} />;
}
export function Mark({ className, faint = false, ...props }: Omit<ComponentProps<"picture">, "children"> & { faint?: boolean; alt?: string }) {
  return <ThemeImage dark={darkMark} white={whiteMark} alt="" className={cn("size-8", faint && "opacity-[0.22]", className)} {...props} />;
}
