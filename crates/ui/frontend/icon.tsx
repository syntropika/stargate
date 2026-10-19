import type { ReactNode } from 'react';

const paths: Record<string, ReactNode> = {
  profile: (
    <>
      <circle cx="9" cy="6" r="3" />
      <path d="M3 16v-1a6 6 0 0 1 12 0v1" />
    </>
  ),
  keys: (
    <>
      <circle cx="6.5" cy="6.5" r="3.5" />
      <path d="m9 9 6 6m-2-2 2-2m-4 0 2-2" />
    </>
  ),
  sessions: (
    <>
      <rect x="2" y="3" width="14" height="10" rx="1" />
      <path d="M6 16h6m-3-3v3" />
    </>
  ),
  users: (
    <>
      <circle cx="7" cy="6" r="2.5" />
      <path d="M2 15v-1a5 5 0 0 1 10 0v1m1-12a3 3 0 0 1 0 6m1 2a4 4 0 0 1 2 4" />
    </>
  ),
  plus: <path d="M9 3v12M3 9h12" />,
  search: (
    <>
      <circle cx="7.5" cy="7.5" r="4.5" />
      <path d="m11 11 4 4" />
    </>
  ),
  shield: (
    <>
      <path d="m9 1 6 3v5c0 4-6 7-6 7S3 13 3 9V4Z" />
      <path d="m6 8 2 2 4-4" />
    </>
  ),
  logout: <path d="M7 3H2v12h5m3-6h6m-3-3 3 3-3 3" />,
};

export function Icon({ name }: { name: keyof typeof paths }) {
  return (
    <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      {paths[name]}
    </svg>
  );
}
