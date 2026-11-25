import hashlib
import io
from pathlib import Path
from zipfile import ZipFile

import pytest

from quickclip321 import media_tools as tools


@pytest.fixture
def setup(tmp_path, monkeypatch):
    monkeypatch.setenv("QUICKCLIP321_DATA_DIR", str(tmp_path))
    monkeypatch.setattr(tools.platform, "system", lambda: "Windows")
    monkeypatch.setattr(tools.platform, "machine", lambda: "AMD64")
    buffer = io.BytesIO()
    with ZipFile(buffer, "w") as archive:
        for name in tools.TOOLS:
            archive.writestr(f"{tools.BUILD}/bin/{name}", name.encode())
        archive.writestr(f"{tools.BUILD}/LICENSE", "license")
        archive.writestr("../unexpected", "not extracted")
    data = buffer.getvalue()
    calls = []

    def open_archive(request, timeout):
        calls.append(request.full_url)
        return io.BytesIO(data)

    monkeypatch.setattr(tools, "urlopen", open_archive)
    monkeypatch.setattr(tools, "ARCHIVE_SIZE", len(data))
    monkeypatch.setattr(tools, "SHA256", hashlib.sha256(data).hexdigest())
    return tmp_path, calls


def test_install_publishes_only_expected_files_and_reuses_them(setup):
    root, calls = setup
    paths = tools.install_media_tools(lambda _: None, lambda: False)
    assert all(Path(path).is_file() for path in paths)
    assert tools.install_media_tools(lambda _: None, lambda: False) == paths
    assert calls == [tools.URL]
    assert not (root / "unexpected").exists()
    assert (Path(paths[0]).parent / "LICENSE").is_file()


def test_unverified_download_is_never_installed(setup, monkeypatch):
    root, _ = setup
    monkeypatch.setattr(tools, "SHA256", "0" * 64)
    with pytest.raises(ValueError, match="verification failed"):
        tools.install_media_tools(lambda _: None, lambda: False)
    assert tools.managed_tools() is None
    assert not (root / "media-tools" / ".installing").exists()


def test_cancel_before_download(setup):
    _, calls = setup
    with pytest.raises(InterruptedError):
        tools.install_media_tools(lambda _: None, lambda: True)
    assert not calls
