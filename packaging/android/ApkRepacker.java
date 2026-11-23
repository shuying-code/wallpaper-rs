import java.io.BufferedOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.util.Enumeration;
import java.util.HashSet;
import java.util.Set;
import java.util.zip.CRC32;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;

/**
 * Merge the live-wallpaper service into the cargo-apk output while forcing
 * resources.arsc to use the ZIP STORED method. Android 11+ rejects APKs whose
 * resource table is deflated; .NET's CompressionLevel.NoCompression still emits
 * a deflate stream, so the Windows packaging script uses this small JDK tool.
 */
public final class ApkRepacker {
    private static final long FIXED_TIME = 1704067200000L;

    private ApkRepacker() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 4) {
            throw new IllegalArgumentException(
                    "usage: ApkRepacker <base.apk> <service.apk> <classes.dex> <out.apk>");
        }
        File base = new File(args[0]);
        File service = new File(args[1]);
        File dex = new File(args[2]);
        File output = new File(args[3]);
        File parent = output.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }

        try (ZipFile serviceZip = new ZipFile(service);
             ZipFile baseZip = new ZipFile(base);
             ZipOutputStream out = new ZipOutputStream(
                     new BufferedOutputStream(new FileOutputStream(output)))) {
            Set<String> serviceNames = new HashSet<>();
            Enumeration<? extends ZipEntry> serviceEntries = serviceZip.entries();
            while (serviceEntries.hasMoreElements()) {
                serviceNames.add(serviceEntries.nextElement().getName());
            }

            ZipEntry arsc = serviceZip.getEntry("resources.arsc");
            if (arsc == null) {
                throw new IllegalStateException("service APK has no resources.arsc");
            }
            copyStored(serviceZip, arsc, out);

            serviceEntries = serviceZip.entries();
            while (serviceEntries.hasMoreElements()) {
                ZipEntry entry = serviceEntries.nextElement();
                if (!"resources.arsc".equals(entry.getName())) {
                    copyDeflated(serviceZip, entry, out);
                }
            }

            Enumeration<? extends ZipEntry> baseEntries = baseZip.entries();
            while (baseEntries.hasMoreElements()) {
                ZipEntry entry = baseEntries.nextElement();
                String name = entry.getName();
                if ("resources.arsc".equals(name)
                        || "classes.dex".equals(name)
                        || serviceNames.contains(name)) {
                    continue;
                }
                copyDeflated(baseZip, entry, out);
            }

            byte[] dexBytes = Files.readAllBytes(dex.toPath());
            ZipEntry dexEntry = new ZipEntry("classes.dex");
            dexEntry.setTime(FIXED_TIME);
            dexEntry.setMethod(ZipEntry.DEFLATED);
            out.putNextEntry(dexEntry);
            out.write(dexBytes);
            out.closeEntry();
        }
    }

    private static void copyStored(ZipFile source, ZipEntry entry, ZipOutputStream out)
            throws Exception {
        byte[] data;
        try (InputStream in = source.getInputStream(entry)) {
            data = readAll(in);
        }
        CRC32 crc = new CRC32();
        crc.update(data);
        ZipEntry target = new ZipEntry(entry.getName());
        target.setTime(FIXED_TIME);
        target.setMethod(ZipEntry.STORED);
        target.setSize(data.length);
        target.setCompressedSize(data.length);
        target.setCrc(crc.getValue());
        out.putNextEntry(target);
        out.write(data);
        out.closeEntry();
    }

    private static void copyDeflated(ZipFile source, ZipEntry entry, ZipOutputStream out)
            throws Exception {
        ZipEntry target = new ZipEntry(entry.getName());
        target.setTime(FIXED_TIME);
        target.setMethod(ZipEntry.DEFLATED);
        out.putNextEntry(target);
        try (InputStream in = source.getInputStream(entry)) {
            copy(in, out);
        }
        out.closeEntry();
    }

    private static byte[] readAll(InputStream in) throws Exception {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        copy(in, out);
        return out.toByteArray();
    }

    private static void copy(InputStream in, OutputStream out) throws Exception {
        byte[] buffer = new byte[64 * 1024];
        int read;
        while ((read = in.read(buffer)) != -1) {
            out.write(buffer, 0, read);
        }
    }
}
