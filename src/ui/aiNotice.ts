import type { AiInventory } from "../types";

export function shouldShowAiNotice(profileCount: number, dismissed: boolean, inventory: AiInventory | null): boolean {
  return profileCount > 0 && !dismissed && inventory !== null &&
    !inventory.clients.some((client) => client.state === "connected");
}
