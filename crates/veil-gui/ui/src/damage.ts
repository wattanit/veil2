// Design §6's verification-failure message, shared so both surfaces that
// can hit a damaged entry — extraction, and preview per Design §8.10's
// "worded exactly as an extraction failure" — say the identical thing
// about it, rather than each composing its own text that could drift.
//
// `removedCopy` is true only for extraction, which always writes and then
// removes a partial destination file on this failure (FR-18). Preview
// never writes anywhere to remove — proved directly by T7.13 and T7.14 —
// so claiming a copy was removed there would describe something that
// never happened.
export function damagedFileMessage(name: string, removedCopy: boolean): string {
  const copyClause = removedCopy ? " The incomplete copy has been removed." : "";
  return `${name} is damaged in the vault.${copyClause} The vault's other files are unaffected.`;
}
