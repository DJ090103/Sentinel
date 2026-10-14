import { tagH } from "./dims";

/** Name pill floating just above the monitor's head, inside the empty top rows of the canvas. */
export const NameTag = ({
  name,
  eyes,
  scale,
  alpha = 1,
}: {
  name: string;
  eyes: string;
  scale: number;
  alpha?: number;
}) => {
  const h = tagH(scale);
  return (
    <div
      className="tag"
      style={{ top: scale * 4, height: h, fontSize: Math.max(10, h * 0.55), opacity: alpha }}
    >
      <span style={{ color: eyes }}>{name}</span>
    </div>
  );
};
