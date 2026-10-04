import type { CSSProperties } from "react";
import type { Profile } from "../types";

export function ProfileDot({ profile }: { profile: Profile }) {
  return <span className="ui-profile-dot" style={{ "--profile-color": profile.accent } as CSSProperties} aria-hidden="true" />;
}
