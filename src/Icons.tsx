import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;
const Base = ({ children, ...props }: IconProps) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
    {...props}
  >
    {children}
  </svg>
);

export const BranchIcon = (props: IconProps) => (
  <Base {...props}><circle cx="6" cy="5" r="2"/><circle cx="18" cy="6" r="2"/><circle cx="6" cy="19" r="2"/><path d="M6 7v10M8 8c5 0 4-2 8-2"/></Base>
);
export const SearchIcon = (props: IconProps) => (
  <Base {...props}><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></Base>
);
export const PlusIcon = (props: IconProps) => (
  <Base {...props}><path d="M12 5v14M5 12h14"/></Base>
);
export const FolderIcon = (props: IconProps) => (
  <Base {...props}><path d="M3 7.5h7l2 2h9v8.5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7.5Z"/><path d="M3 7.5v-1a2 2 0 0 1 2-2h4l2 2"/></Base>
);
export const CheckIcon = (props: IconProps) => (
  <Base {...props}><path d="m5 12 4 4L19 6"/></Base>
);
export const AlertIcon = (props: IconProps) => (
  <Base {...props}><path d="M12 8v5M12 17h.01"/><path d="M10.3 3.7 2.6 17a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 3.7a2 2 0 0 0-3.4 0Z"/></Base>
);
export const KeyIcon = (props: IconProps) => (
  <Base {...props}><circle cx="8" cy="15" r="4"/><path d="m11 12 8-8M16 7l2 2M14 9l2 2"/></Base>
);
export const TerminalIcon = (props: IconProps) => (
  <Base {...props}><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/></Base>
);
export const ChevronIcon = (props: IconProps) => (
  <Base {...props}><path d="m9 18 6-6-6-6"/></Base>
);
export const MoreIcon = (props: IconProps) => (
  <Base {...props}><circle cx="5" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="19" cy="12" r="1" fill="currentColor" stroke="none"/></Base>
);
export const CloseIcon = (props: IconProps) => (
  <Base {...props}><path d="m6 6 12 12M18 6 6 18"/></Base>
);
export const ShieldIcon = (props: IconProps) => (
  <Base {...props}><path d="M12 3 5 6v5c0 4.6 2.8 8.1 7 10 4.2-1.9 7-5.4 7-10V6l-7-3Z"/><path d="m9 12 2 2 4-4"/></Base>
);
export const EditIcon = (props: IconProps) => (
  <Base {...props}><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4Z"/></Base>
);
export const TrashIcon = (props: IconProps) => (
  <Base {...props}><path d="M4 7h16M9 7V4h6v3M6 7l1 14h10l1-14M10 11v6M14 11v6"/></Base>
);
