import asyncio

from canvas_protocol import CanvasClient


async def main():
    # Server URI
    uri = "ws://localhost:2325"

    client = CanvasClient(uri)
    async with client:
        print(f"Connected to canvas server at {uri}")

        print("\nGetting all canvas pixels...")
        await client.get_all_pixels()
        all_pixels = await client.receive_all_pixels()
        print(f"   Received {len(all_pixels)} total pixels from canvas")

        if len(all_pixels) > 0:
            print(
                f"   First pixel: RGB({all_pixels[0].r}, {all_pixels[0].g}, {all_pixels[0].b})"
            )
            if len(all_pixels) > 1:
                print(
                    f"   Last pixel: RGB({all_pixels[-1].r}, {all_pixels[-1].g}, {all_pixels[-1].b})"
                )


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:
        print(f"\nUnexpected error: {e}")
