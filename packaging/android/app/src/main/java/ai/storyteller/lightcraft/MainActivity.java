// LightCraft on Android: a NativeActivity running the Rust app in libmain.so
// (apps/lightcraft-android). This class only does what needs the Java side; the Rust code calls
// these methods over JNI (apps/lightcraft-android/src/ffi.rs).
package ai.storyteller.lightcraft;

import android.Manifest;
import android.app.NativeActivity;
import android.content.ClipData;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.media.MediaScannerConnection;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.OpenableColumns;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class MainActivity extends NativeActivity {
    private static final String TAG = "lightcraft";
    private static final int PICK_PHOTOS = 1;
    /** Raw formats some galleries don't label as images. */
    private static final String[] MIME_TYPES = {
        "image/*", "application/octet-stream", "application/x-dng",
    };

    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final Object lock = new Object();
    /** The picker is open, or picked photos are still being copied. */
    private boolean picking = false;
    /** Paths of the copied photos, one per line, until takePicked() hands them over. */
    private final StringBuilder picked = new StringBuilder();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // before the native library starts: it reads these at launch
        setupEnvironment();
        super.onCreate(savedInstanceState);
        if (Build.VERSION.SDK_INT == Build.VERSION_CODES.Q
                && checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) != PackageManager.PERMISSION_GRANTED) {
            // Android 10 only: exports go to the shared Pictures folder
            requestPermissions(new String[] {Manifest.permission.WRITE_EXTERNAL_STORAGE}, 2);
        }
    }

    private void setupEnvironment() {
        try {
            // settings (~/.config/lightcraft), temporary files
            Os.setenv("HOME", getFilesDir().getAbsolutePath(), true);
            Os.setenv("TMPDIR", getCacheDir().getAbsolutePath(), true);
            // the library (catalog, previews, imported originals): the app's external files folder,
            // reachable over USB as Android/data/ai.storyteller.lightcraft/files
            File ext = getExternalFilesDir(null);
            File base = ext != null ? ext : getFilesDir();
            Os.setenv("LIGHTCRAFT_LIBRARY", new File(base, "LightCraft Library").getAbsolutePath(), true);
            // exports land in the gallery: Pictures/LightCraft
            File pictures = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES);
            Os.setenv("LIGHTCRAFT_EXPORT_DIR", new File(pictures, "LightCraft").getAbsolutePath(), true);
        } catch (ErrnoException e) {
            Log.w(TAG, "setenv failed", e);
        }
    }

    /** Where picked photos are kept: they are the originals the library refers to. */
    private File photosDir() {
        File ext = getExternalFilesDir(null);
        return new File(new File(ext != null ? ext : getFilesDir(), "LightCraft Library"), "Originals/Picked");
    }

    // ---------------------------------------------------------------- called from Rust

    /** Open the system picker for photos and raw files. */
    public void pickPhotos() {
        synchronized (lock) {
            picking = true;
        }
        runOnUiThread(() -> {
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType("*/*");
            i.putExtra(Intent.EXTRA_MIME_TYPES, MIME_TYPES);
            i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            try {
                startActivityForResult(i, PICK_PHOTOS);
            } catch (RuntimeException e) {
                Log.w(TAG, "no document picker", e);
                synchronized (lock) {
                    picking = false;
                }
            }
        });
    }

    /** null while the picker is open or copies are being made; else the copied paths, one per line. */
    public String takePicked() {
        synchronized (lock) {
            if (picking) {
                return null;
            }
            String s = picked.toString();
            picked.setLength(0);
            return s;
        }
    }

    /** Make a written file show up in the gallery. */
    public void mediaScan(String path) {
        MediaScannerConnection.scanFile(getApplicationContext(), new String[] {path}, null, null);
    }

    /** Open a link in the browser. */
    public void openUrl(String url) {
        runOnUiThread(() -> {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            } catch (RuntimeException e) {
                Log.w(TAG, "no browser for " + url, e);
            }
        });
    }

    // ---------------------------------------------------------------- picker results

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != PICK_PHOTOS) {
            return;
        }
        List<Uri> uris = new ArrayList<>();
        if (resultCode == RESULT_OK && data != null) {
            ClipData clip = data.getClipData();
            if (clip != null) {
                for (int i = 0; i < clip.getItemCount(); i++) {
                    uris.add(clip.getItemAt(i).getUri());
                }
            } else if (data.getData() != null) {
                uris.add(data.getData());
            }
        }
        // copied off the UI thread; takePicked() returns null until they are all done
        io.execute(() -> {
            StringBuilder paths = new StringBuilder();
            for (Uri uri : uris) {
                try {
                    paths.append(copy(uri).getAbsolutePath()).append('\n');
                } catch (IOException | RuntimeException e) {
                    Log.w(TAG, "copying " + uri + " failed", e);
                }
            }
            synchronized (lock) {
                picked.append(paths);
                picking = false;
            }
        });
    }

    /** Copy a picked document into photosDir() under its own name (made unique if taken). */
    private File copy(Uri uri) throws IOException {
        File dir = photosDir();
        if (!dir.isDirectory() && !dir.mkdirs()) {
            throw new IOException("can't create " + dir);
        }
        String name = displayName(uri);
        File out = new File(dir, name);
        int dot = name.lastIndexOf('.');
        String stem = dot > 0 ? name.substring(0, dot) : name;
        String ext = dot > 0 ? name.substring(dot) : "";
        for (int n = 2; out.exists(); n++) {
            out = new File(dir, stem + "-" + n + ext);
        }
        File tmp = new File(dir, "." + out.getName() + ".part");
        try (InputStream in = getContentResolver().openInputStream(uri);
             OutputStream os = new FileOutputStream(tmp)) {
            if (in == null) {
                throw new IOException("can't open " + uri);
            }
            byte[] buf = new byte[1 << 16];
            int r;
            while ((r = in.read(buf)) > 0) {
                os.write(buf, 0, r);
            }
        } catch (IOException e) {
            tmp.delete();
            throw e;
        }
        if (!tmp.renameTo(out)) {
            tmp.delete();
            throw new IOException("can't write " + out);
        }
        return out;
    }

    /** The document's file name, without path separators. */
    private String displayName(Uri uri) {
        String name = null;
        try (Cursor c = getContentResolver().query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                name = c.getString(0);
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "no name for " + uri, e);
        }
        if (name == null || name.isEmpty()) {
            name = "photo-" + System.currentTimeMillis();
        }
        return name.replace('/', '_').replace('\\', '_');
    }
}
