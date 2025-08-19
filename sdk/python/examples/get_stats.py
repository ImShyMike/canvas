import asyncio

from canvas_protocol import CanvasClient


async def main():
    # Server URI
    uri = "ws://localhost:2325"

    client = CanvasClient(uri)
    async with client:
        print(f"Connected to canvas server at {uri}")

        print("\nGetting server statistics...")
        await client.get_stats()
        stats = await client.receive_stats()
        print(f"   Connected clients: {stats.connected_clients}")
        print(f"   Requests per second: {stats.requests_per_second:.2f}")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except ConnectionError as e:
        print(f"\nConnection error: {e}")
        print("Make sure the canvas server is running!")
    except Exception as e:
        print(f"\nUnexpected error: {e}")
