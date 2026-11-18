import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, renderHook, screen } from '@testing-library/react';
import { Link } from './Link';
import { navigate, normalizePath, usePathname } from './router';

beforeEach(() => {
  window.history.replaceState(null, '', '/');
  vi.spyOn(window, 'scrollTo').mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
  window.history.replaceState(null, '', '/');
});

describe('normalizePath', () => {
  it.each([
    ['/', '/'],
    ['', '/'],
    ['/workspace', '/workspace'],
    ['/workspace/', '/workspace'],
    ['/workspace///', '/workspace'],
  ])('normalises %j to %j', (input, expected) => {
    expect(normalizePath(input)).toBe(expected);
  });
});

describe('navigate / usePathname', () => {
  it('pushes history and notifies subscribers', () => {
    const { result } = renderHook(() => usePathname());
    expect(result.current).toBe('/');

    act(() => navigate('/workspace'));

    expect(window.location.pathname).toBe('/workspace');
    expect(result.current).toBe('/workspace');
    expect(window.history.length).toBeGreaterThan(0);
    expect(window.scrollTo).toHaveBeenCalledWith(0, 0);
  });

  it('replaces history when requested', () => {
    const before = window.history.length;
    act(() => navigate('/workspace', { replace: true }));
    expect(window.history.length).toBe(before);
    expect(window.location.pathname).toBe('/workspace');
  });

  it('reacts to popstate (browser back/forward)', () => {
    const { result } = renderHook(() => usePathname());
    act(() => {
      window.history.pushState(null, '', '/elsewhere');
      window.dispatchEvent(new PopStateEvent('popstate'));
    });
    expect(result.current).toBe('/elsewhere');
  });

  it('scrolls to a hash target after navigation', () => {
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) => {
      cb(0);
      return 0;
    });
    const target = document.createElement('section');
    target.id = 'workflow';
    target.scrollIntoView = vi.fn();
    document.body.appendChild(target);

    act(() => navigate('/#workflow'));

    expect(target.scrollIntoView).toHaveBeenCalled();
    expect(window.scrollTo).not.toHaveBeenCalled();
    target.remove();
  });

  it('does not push a duplicate entry for the current location', () => {
    const spy = vi.spyOn(window.history, 'pushState');
    act(() => navigate('/'));
    expect(spy).not.toHaveBeenCalled();
  });
});

describe('Link', () => {
  // jsdom cannot perform real navigations; swallow the browser default *after*
  // Link has decided whether to handle the click itself.
  const preventBrowserNavigation = (event: Event): void => event.preventDefault();
  beforeEach(() => document.addEventListener('click', preventBrowserNavigation));
  afterEach(() => document.removeEventListener('click', preventBrowserNavigation));

  it('navigates client-side on plain left click', () => {
    render(<Link to="/workspace">Workspace</Link>);
    fireEvent.click(screen.getByRole('link', { name: 'Workspace' }));
    expect(window.location.pathname).toBe('/workspace');
  });

  it.each([{ metaKey: true }, { ctrlKey: true }, { shiftKey: true }, { button: 1 }])(
    'leaves modified clicks %j to the browser',
    (modifier) => {
      render(<Link to="/workspace">Workspace</Link>);
      const link = screen.getByRole('link', { name: 'Workspace' });
      const event = new MouseEvent('click', { bubbles: true, cancelable: true, ...modifier });
      link.dispatchEvent(event);
      expect(window.location.pathname).toBe('/');
    },
  );

  it('does not intercept links that open in a new tab', () => {
    render(
      <Link to="/workspace" target="_blank">
        Workspace
      </Link>,
    );
    fireEvent.click(screen.getByRole('link', { name: 'Workspace' }));
    expect(window.location.pathname).toBe('/');
  });

  it('calls a provided onClick handler', () => {
    const onClick = vi.fn();
    render(
      <Link to="/workspace" onClick={onClick}>
        Workspace
      </Link>,
    );
    fireEvent.click(screen.getByRole('link', { name: 'Workspace' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
