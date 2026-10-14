/** Canvas size in art pixels; the window is exactly this times `scale`, plus the name tag above. */
export const W = 40;
export const H = 44;
/** Height in screen px of the name tag that sits above the head. */
export const tagH = (scale: number) => Math.round(scale * 6);
