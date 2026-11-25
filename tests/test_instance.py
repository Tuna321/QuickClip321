from quickclip321.instance import SingleInstance
import pytest


def test_second_instance_cannot_create_another_tray(tmp_path):
    path = tmp_path / "single.lock"
    first = SingleInstance(path)
    try:
        with pytest.raises(RuntimeError):
            SingleInstance(path)
    finally:
        first.close()
    again = SingleInstance(path)
    again.close()
