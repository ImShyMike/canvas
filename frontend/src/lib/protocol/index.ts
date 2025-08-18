export function packCoordinates(x: number, y: number): Uint8Array {
	const packed = (x << 10) | y;
	return new Uint8Array([(packed >> 16) & 0xff, (packed >> 8) & 0xff, packed & 0xff]);
}

export function unpackCoordinates(bytes: Uint8Array): [number, number] {
	const packed = (bytes[0] << 16) | (bytes[1] << 8) | bytes[2];
	const x = (packed >> 10) & 0x3ff;
	const y = packed & 0x3ff;
	return [x, y];
}

export function packRGB(color: number): Uint8Array {
	return new Uint8Array([(color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff]);
}

export function unpackRGB(bytes: Uint8Array): number {
	return (bytes[0] << 16) | (bytes[1] << 8) | bytes[2];
}

export const MessageType = {
	SET_PIXEL: 1,
	GET_PIXEL: 2,
	GET_ALL_PIXELS: 3,
	GET_STATS: 4
};

export const ResponseType = {
	PIXEL_COLOR: 10,
	ERROR: 11,
	GET_ALL_PIXELS: 12,
	STATS: 13
};
