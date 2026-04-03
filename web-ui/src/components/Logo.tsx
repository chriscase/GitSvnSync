import { useId } from 'react';

interface LogoSvgProps {
  size?: number;
  className?: string;
}

export function LogoSvg({ size = 32, className }: LogoSvgProps) {
  const id = useId();
  const gradId = `logoGrad${id}`;
  const arrowId = `arrowGrad${id}`;

  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 56 56"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id={gradId} x1="0" y1="0" x2="56" y2="56" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#3b82f6" />
          <stop offset="100%" stopColor="#818cf8" />
        </linearGradient>
        <linearGradient id={arrowId} x1="10" y1="10" x2="46" y2="46" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#60a5fa" />
          <stop offset="100%" stopColor="#a78bfa" />
        </linearGradient>
      </defs>
      <circle cx="28" cy="28" r="25" stroke={`url(#${gradId})`} strokeWidth="2" fill="none" opacity="0.3" />
      <path d="M 38 14 A 16 16 0 0 1 42 28" stroke={`url(#${arrowId})`} strokeWidth="2.5" strokeLinecap="round" fill="none" />
      <polygon points="43,27 42,31 39,28" fill="#60a5fa" />
      <path d="M 18 42 A 16 16 0 0 1 14 28" stroke={`url(#${arrowId})`} strokeWidth="2.5" strokeLinecap="round" fill="none" />
      <polygon points="13,29 14,25 17,28" fill="#a78bfa" />
      <circle cx="28" cy="28" r="5" fill="#1e293b" stroke={`url(#${gradId})`} strokeWidth="2" />
      <circle cx="28" cy="28" r="2" fill="#60a5fa" />
      <line x1="28" y1="23" x2="28" y2="14" stroke="#3b82f6" strokeWidth="1.5" strokeLinecap="round" opacity="0.7" />
      <line x1="28" y1="33" x2="28" y2="42" stroke="#818cf8" strokeWidth="1.5" strokeLinecap="round" opacity="0.7" />
      <circle cx="28" cy="13" r="2" fill="#3b82f6" />
      <circle cx="28" cy="43" r="2" fill="#818cf8" />
    </svg>
  );
}
