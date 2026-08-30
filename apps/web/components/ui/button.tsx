import type { ButtonHTMLAttributes, Ref } from "react";
import styles from "./button.module.css";

type Variant = "default" | "primary" | "quiet";

/**
 * A real `<button>`: keyboard operability, focus, and disabled semantics
 * come from the platform rather than being reimplemented.
 */
export function Button({
  variant = "default",
  className,
  type = "button",
  ref,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  /** React 19 passes refs as a normal prop; no `forwardRef` needed. */
  ref?: Ref<HTMLButtonElement>;
}) {
  const variantClass =
    variant === "primary" ? styles.primary : variant === "quiet" ? styles.quiet : "";

  return (
    <button
      ref={ref}
      type={type}
      className={[styles.button, variantClass, className].filter(Boolean).join(" ")}
      {...props}
    />
  );
}

export { styles as buttonStyles };
