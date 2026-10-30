import type { Component } from "svelte";
import type { SVGAttributes } from "svelte/elements";

/** An icon component from lib/icons.ts, passed as a prop to buttons, nav items and fields. */
export type IconComponent = Component<SVGAttributes<SVGSVGElement> & { size?: number | string }>;
