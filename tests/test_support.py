from quickclip321.support import redact_support_text


def test_support_export_removes_private_urls_paths_and_credentials():
    raw = (
        "visit https://site.example/private/video?token=abc123\n"
        "output C:\\Users\\Alice\\Downloads\\private video.mp4\n"
        "share \\\\server\\private\\secret.txt\n"
        "Cookie: session=topsecret\n"
        "error NETWORK_TIMEOUT"
    )
    safe = redact_support_text(raw)
    for secret in ("private/video", "abc123", "Alice", "server", "topsecret"):
        assert secret not in safe
    assert "NETWORK_TIMEOUT" in safe
