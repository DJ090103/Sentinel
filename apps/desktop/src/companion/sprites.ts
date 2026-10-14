import type { Cheek, Eye, Fx, Mouth } from "./face";

/**
 * Original pixel art, drawn as text. One char = one art pixel.
 * '#' feature colour (tinted by heat)  w white  p pink  b tear/blue  d dark  r red
 * o orange  y yellow  g grey  c cool cyan  '.' empty
 * Eyes are 5x6 (left eye; the right eye is mirrored). Mouths 8x4. Cheeks 4x2.
 */
export const EYES: Record<Exclude<Eye, "none">, string[]> = {
  normal: [".###.", "#w###", "#w###", "#####", "#####", ".###."],
  red: [".###.", "#####", "#####", "#####", "#####", ".###."],
  happy: [".....", ".###.", "##.##", "#...#", ".....", "....."],
  sleepy: [".....", ".....", "#####", "#w###", ".###.", "....."],
  closed: [".....", ".....", ".....", "#####", ".....", "....."],
  blink: [".....", ".....", "#####", "#####", ".....", "....."],
  surprised: [".###.", "#...#", "#.#.#", "#.#.#", "#...#", ".###."],
  focused: [".....", "#####", ".###.", ".###.", ".....", "....."],
  angry: ["##...", "####.", "#####", ".####", "..###", "....."],
  stressed: ["##...", ".##..", "..###", ".##..", "##...", "....."],
  down: ["#####", ".....", ".###.", ".###.", "..#..", "....."],
  heart: [".p.p.", "ppppp", "ppppp", ".ppp.", "..p..", "....."],
  crying: [".....", ".....", ".###.", "#...#", "b...b", "b...b"],
};

export const MOUTHS: Record<Exclude<Mouth, "none">, string[]> = {
  neutral: ["........", "........", "..####..", "........"],
  smile: ["........", ".#....#.", "..####..", "........"],
  happy: ["#......#", "##....##", ".######.", "........"],
  big: ["########", "#dddddd#", ".#dddd#.", "..####.."],
  laughing: ["########", "#dddddd#", ".#dppd#.", "..####.."],
  sleepy: ["........", "........", "...###..", "........"],
  yawn: ["..####..", ".#dddd#.", ".#dddd#.", "..####.."],
  surprised: ["........", "...##...", "..#dd#..", "...##..."],
  open: ["........", "..####..", "..#dd#..", "..####.."],
  stressed: ["........", ".#.##.#.", "#.#..#.#", "........"],
};

export const CHEEK: Record<Exclude<Cheek, "none">, string[]> = {
  light: [".pp.", "pppp"],
  strong: [".pp.", "pppp"],
  red: [".pp.", "pppp"],
};

/** 28-wide overlay drawn from screen row 2: glasses frame around both eyes. */
export const GLASSES = [
  "....#######......#######....",
  "....#.....#......#.....#....",
  "#####.....########.....#####",
  "....#.....#......#.....#....",
  "....#.....#......#.....#....",
  "....#.....#......#.....#....",
  "....#.....#......#.....#....",
  "....#######......#######....",
];

const SPR = {
  heart: [".p.p.", "ppppp", ".ppp.", "..p.."],
  drop: [".b.", "bbb", "bbb", ".b."],
  spark: ["..y..", "..y..", "yyyyy", "..y..", "..y.."],
  z: ["wwww", "..w.", ".w..", "wwww"],
  puff: [".g.", "ggg", ".g."],
  wave: [".o.", "o..", ".o.", "..o", ".o.", "o..", ".o.", "..o"],
  flame: ["...o..", "...oo.", "..ooo.", ".oooyo", ".ooyyo", "ooyyyo", "ooyyyo", ".oooo."],
  dots: [".wwwwwww.", "wwwwwwwww", "wwdwdwdww", "wwwwwwwww", ".wwwwwww.", "..ww.....", "..w......"],
  bang: [".wwwwwww.", "wwwwrwwww", "wwwwrwwww", "wwwwrwwww", ".wwwwwww.", "..ww.....", "..w......"],
  anger: ["r...r", ".r.r.", "..r..", ".r.r.", "r...r"],
  wind: ["cccccc", ".cccc."],
  note: ["..yyy", "..y.y", "..y..", "yyy..", "yy..."],
};
export type SpriteName = keyof typeof SPR;
export const sprite = (n: SpriteName) => SPR[n];

export const FX_LIST: Fx[] = [
  "hearts",
  "sweat",
  "heat",
  "steam",
  "sparkles",
  "zzz",
  "bubble",
  "anger",
  "wind",
  "flame",
  "glitch",
  "notes",
];
