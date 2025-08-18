export function getRandomHexColor(): string {
	return Math.floor(Math.random() * 0xffffff)
		.toString(16)
		.padStart(6, '0');
}

export function hexToRGB(hex: string): number {
	const r = parseInt(hex.slice(1, 3), 16);
	const g = parseInt(hex.slice(3, 5), 16);
	const b = parseInt(hex.slice(5, 7), 16);
	return (r << 16) | (g << 8) | b;
}

export function rgbToHex(rgb: number): string {
	return '#' + rgb.toString(16).padStart(6, '0');
}
