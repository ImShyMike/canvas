"""Demo to run on the actual canvas server."""

import asyncio
import random
import time

from canvas_protocol import CanvasClient, Color


async def main():
    """Main demo function."""

    uri = "wss://canvas-api.shymike.dev"

    client = CanvasClient(uri)

    async with client:
        print(f"Connected to canvas server at {uri}")

        width, height = 1000, 1000
        square_size = 25
        while True:
            start_x = random.randint(0, width - square_size)
            start_y = random.randint(0, height - square_size)
            color = Color(
                random.randint(0, 255), random.randint(0, 255), random.randint(0, 255)
            )

            for y in range(start_y, start_y + square_size):
                for x in range(start_x, start_x + square_size):
                    await client.set_pixel(x, y, color, confirmation=False)
                    time.sleep(0.01)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:  # pylint: disable=broad-except
        print(f"\nUnexpected error: {e}")
