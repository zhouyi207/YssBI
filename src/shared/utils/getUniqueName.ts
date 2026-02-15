export function getUniqueName(
  baseName: string,
  items: Iterable<{ name: string }>
) {
  const used = new Set<number>();
  let hasBase = false;

  const pattern = new RegExp(`^${baseName}(?:_(\\d+))?$`);

  for (const { name } of items) {
    const match = name.match(pattern);
    if (!match) continue;

    if (match[1] === undefined) {
      hasBase = true;
    } else {
      used.add(Number(match[1]));
    }
  }

  if (!hasBase) return baseName;

  let i = 1;
  while (used.has(i)) i++;

  return `${baseName}_${i}`;
}
