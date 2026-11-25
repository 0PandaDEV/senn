export type IconName = "x" | "folder" | "folder-open" | "file" | "up" | "pencil" | "trash" | "chevron" | "plus" | "check" | "alert" | "info";

export function icon(name: IconName, size = 16): string {
  return `<svg class="icon" width="${size}" height="${size}" aria-hidden="true"><use href="#i-${name}"></use></svg>`;
}

export {};
