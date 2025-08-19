import asyncio

from canvas_protocol import (
    AllPixelsData,
    CanvasClient,
    ErrorData,
    PixelColorData,
    SetPixelBroadcastData,
    Stats,
    parse_message,
)


async def main():
    # Server URI
    uri = "ws://localhost:2325"

    client = CanvasClient(uri)
    async with client:
        print(f"Connected to canvas server at {uri}")

        update_count = 0
        async for response_type, data in client.listen_for_messages():
            update_count += 1
            parsed_data = parse_message(response_type, data)
            if isinstance(parsed_data, SetPixelBroadcastData):
                print(
                    f"Received pixel update: {parsed_data.x}, {parsed_data.y} = {parsed_data.color}"
                )
            elif isinstance(parsed_data, PixelColorData):
                print(f"Received pixel color: {parsed_data.color}")
            elif isinstance(parsed_data, ErrorData):
                print(f"Error received: {parsed_data.message}")
            elif isinstance(parsed_data, Stats):
                print(
                    f"Stats update: {parsed_data.connected_clients} clients connected, {parsed_data.requests_per_second:.2f} requests/sec"
                )
            elif isinstance(parsed_data, AllPixelsData):
                print(f"Received all pixels data with {len(parsed_data.pixels)} pixels")
            else:
                print(f"Unknown response type: {response_type}")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:
        print(f"\nUnexpected error: {e}")
