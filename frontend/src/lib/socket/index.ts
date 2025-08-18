import type { Socket } from './socket';
import { MessageType } from '$lib/protocol';
import { packCoordinates, packRGB } from '$lib/protocol';

export type { Socket };

export function connectWebSocket(
	socket: Socket,
	websocket_url: string,
	onMessage: (data: Uint8Array) => void
) {
	try {
		socket.ws = new WebSocket(websocket_url);

		socket.ws.onopen = () => {
			console.log('Connected to WebSocket');
			socket.isConnected = true;
			requestAllPixels(socket);
		};

		socket.ws.onclose = () => {
			console.log('WebSocket connection closed');
			socket.isConnected = false;
			// try to reconnect after 3 seconds
			setTimeout(() => connectWebSocket(socket, websocket_url, onMessage), 3000);
		};

		socket.ws.onerror = (error) => {
			console.error('WebSocket error:', error);
			socket.isConnected = false;
		};

		socket.ws.onmessage = (event) => {
			if (event.data instanceof Blob) {
				event.data.arrayBuffer().then((buffer) => {
					onMessage(new Uint8Array(buffer));
				});
			}
		};
	} catch (error) {
		console.error('Failed to connect to WebSocket:', error);
		socket.isConnected = false;
	}
}

export function sendSetPixel(socket: Socket, x: number, y: number, color: number) {
	if (!socket.isConnected || !socket.ws) return;

	const message = new Uint8Array(7);
	message[0] = MessageType.SET_PIXEL;

	const coords = packCoordinates(x, y);
	message.set(coords, 1);

	const rgb = packRGB(color);
	message.set(rgb, 4);

	socket.ws.send(message);
}

export function requestAllPixels(socket: Socket) {
	if (!socket.isConnected || !socket.ws) return;

	const message = new Uint8Array(1);
	message[0] = MessageType.GET_ALL_PIXELS;
	socket.ws.send(message);
}
