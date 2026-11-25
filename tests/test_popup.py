import pytest
from quickclip321.popup import Rect, placement, should_dismiss


@pytest.mark.parametrize(
    "monitor,work,anchor,expected_edge",
    [
        (Rect(0, 0, 1920, 1080), Rect(0, 0, 1920, 1032), Rect(1820, 1040, 1844, 1064), "bottom"),
        (Rect(0, 0, 1920, 1080), Rect(0, 48, 1920, 1080), Rect(1800, 8, 1824, 32), "top"),
        (Rect(0, 0, 1920, 1080), Rect(48, 0, 1920, 1080), Rect(8, 950, 32, 974), "left"),
        (Rect(0, 0, 1920, 1080), Rect(0, 0, 1872, 1080), Rect(1880, 950, 1904, 974), "right"),
        (Rect(-1920, 0, 0, 1080), Rect(-1920, 0, 0, 1032), Rect(-100, 1040, -76, 1064), "bottom"),
        (Rect(1920, -1440, 4480, 0), Rect(1920, -1440, 4480, -60), Rect(4350, -50, 4380, -20), "bottom"),
    ],
)
@pytest.mark.parametrize("dpi", [96, 144, 192])
def test_tray_anchor_monitor_edges_and_scaling(monitor, work, anchor, expected_edge, dpi):
    result = placement(anchor, monitor, work, (round(600 * dpi / 96), round(294 * dpi / 96)), dpi)
    assert work.left <= result.left < result.right <= work.right
    assert work.top <= result.top < result.bottom <= work.bottom
    if expected_edge == "bottom":
        assert result.bottom <= anchor.top
    if expected_edge == "top":
        assert result.top >= anchor.bottom
    if expected_edge == "left":
        assert result.left >= anchor.right
    if expected_edge == "right":
        assert result.right <= anchor.left


def test_oversized_full_view_is_clamped():
    assert placement(Rect(900, 700, 924, 724), Rect(0, 0, 1024, 768), Rect(0, 0, 1024, 728), (1900, 1500)) == Rect(
        0, 0, 1024, 728
    )


@pytest.mark.parametrize("change", [{"inside": True}, {"owned": True}, {"on_tray": True}, {"pressed": False}])
def test_safe_clicks_do_not_dismiss(change):
    state = {"pressed": True, "inside": False, "owned": False, "on_tray": False, **change}
    assert not should_dismiss(state, False)


def test_outside_click_and_modal_guard():
    state = {"pressed": True, "inside": False, "owned": False, "on_tray": False}
    assert should_dismiss(state, False)
    assert not should_dismiss(state, True)
    assert not should_dismiss(state, False, dialog_open=True)
