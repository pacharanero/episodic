// Path data from Simple Icons 16.38.1 (CC0-1.0), https://simpleicons.org.
// Lucide no longer ships brand marks. YouTube and YouTube Studio are
// trademarks of Google LLC, used here only to label links to those services.

interface IconProps {
  size?: number;
}

export function YoutubeIcon({ size = 18 }: IconProps) {
  return <svg className="brand-icon youtube" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path fill="currentColor" d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z" />
  </svg>;
}

export function YoutubeStudioIcon({ size = 18 }: IconProps) {
  return <svg className="brand-icon youtube" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path fill="currentColor" d="M20.919 13.176c.048-.384.084-.768.084-1.176s-.036-.792-.084-1.176l2.532-1.98a.605.605 0 0 0 .144-.768l-2.4-4.152a.603.603 0 0 0-.732-.264l-2.988 1.2a8.767 8.767 0 0 0-2.028-1.176l-.456-3.18A.585.585 0 0 0 14.403 0h-4.8c-.3 0-.552.216-.588.504l-.456 3.18A9.22 9.22 0 0 0 6.531 4.86l-2.988-1.2a.585.585 0 0 0-.732.264l-2.4 4.152a.592.592 0 0 0 .144.768l2.532 1.98c-.048.384-.084.78-.084 1.176s.036.792.084 1.176l-2.532 1.98a.605.605 0 0 0-.144.768l2.4 4.152c.144.264.468.36.732.264l2.988-1.2c.624.48 1.296.876 2.028 1.176l.456 3.18a.585.585 0 0 0 .588.504h4.8c.3 0 .552-.216.588-.504l.456-3.18a9.22 9.22 0 0 0 2.028-1.176l2.988 1.2c.276.108.588 0 .732-.264l2.4-4.152a.605.605 0 0 0-.144-.768l-2.532-1.98zM9.603 15.6V8.4l6 3.6-6 3.6z" />
  </svg>;
}
