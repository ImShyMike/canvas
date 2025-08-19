import asyncio
import random

from canvas_protocol import CanvasClient


async def main():
    # Server URI
    uri = "ws://localhost:2325"

    client = CanvasClient(uri)
    async with client:
        print(f"Connected to canvas server at {uri}")

        print("\nReading random pixels...")
        test_positions = [
            (random.randint(0, 1024), random.randint(0, 1024)) for _ in range(10)
        ]

        for x, y in test_positions:
            await client.get_pixel(x, y)
            received_color = await client.receive_pixel_color()
            print(f"   Pixel at ({x}, {y}): {received_color}")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:
        print(f"\nUnexpected error: {e}")
