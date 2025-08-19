import asyncio

from canvas_protocol import CanvasClient, Color


async def main():
    # Server URI
    uri = "ws://localhost:2325"

    client = CanvasClient(uri)
    async with client:
        print(f"Connected to canvas server at {uri}")

        square_size = 250
        start_x, start_y = 50, 50
        color = Color(150, 50, 200)

        print(
            f"\nDrawing a {square_size}x{square_size} square at position ({start_x}, {start_y}) with color {color}"
        )

        pixel_count = 0
        for y in range(start_y, start_y + square_size):
            for x in range(start_x, start_x + square_size):
                # Confirmation is not needed for this example
                await client.set_pixel(x, y, color, confirmation=False)
                pixel_count += 1

                if pixel_count % (square_size) == 0:
                    print(
                        f"   Set pixel ({x}, {y}) to RGB({color.r}, {color.g}, {color.b})"
                    )

        print(f"Drew {pixel_count} pixels!")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:
        print(f"\nUnexpected error: {e}")
