import receipt from "./assets/receipt.png";

/** The paper of the cafe sample, as the default printer draws it: `pnpm receipt` makes the picture. */
export function Receipt() {
  return (
    <div className="receipt relative w-[552px] max-w-full bg-paper px-6 pt-6 pb-4 shadow-[0_16px_40px_rgb(0_0_0/0.5)]">
      <img
        src={receipt}
        width={1008}
        height={1064}
        alt="The receipt of Corner Cafe: the shop, an order number, three items with their prices and a total of 16.52."
        className="block w-full"
      />
    </div>
  );
}
