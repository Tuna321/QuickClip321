import pytest

from quickclip321.validation import validate_media_url


@pytest.mark.parametrize("value", ["", "selam bro", "youtube video", "javascript:alert(1)", "C:\\video.mp4"])
def test_rejects_random_text(value):
    valid, normalized, error = validate_media_url(value)
    assert not valid
    assert not normalized
    assert error


@pytest.mark.parametrize(
    "value, expected",
    [
        ("https://www.youtube.com/watch?v=abc123", "https://www.youtube.com/watch?v=abc123"),
        ("www.tiktok.com/@demo/video/123", "https://www.tiktok.com/@demo/video/123"),
        ("https://vimeo.com/123", "https://vimeo.com/123"),
    ],
)
def test_accepts_web_urls(value, expected):
    valid, normalized, error = validate_media_url(value)
    assert valid
    assert normalized == expected
    assert not error


def test_rejects_credentials_and_ip_addresses():
    assert not validate_media_url("https://user:pass@example.com/video")[0]
    assert not validate_media_url("http://127.0.0.1/video")[0]
