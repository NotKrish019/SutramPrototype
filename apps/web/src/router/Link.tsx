import React from 'react';
import { navigate } from './router';

export interface LinkProps extends Omit<React.AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> {
  /** Internal path, optionally with a hash, e.g. `/workspace` or `/#workflow`. */
  to: string;
}

/**
 * Anchor that performs client-side navigation for plain left clicks and falls
 * back to normal browser behaviour for modified clicks (new tab, etc.).
 */
export const Link = React.forwardRef<HTMLAnchorElement, LinkProps>(function Link(
  { to, onClick, target, ...rest },
  ref,
) {
  const handleClick = (event: React.MouseEvent<HTMLAnchorElement>): void => {
    onClick?.(event);
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey ||
      (target !== undefined && target !== '_self')
    ) {
      return;
    }
    event.preventDefault();
    navigate(to);
  };

  return <a ref={ref} href={to} target={target} onClick={handleClick} {...rest} />;
});
