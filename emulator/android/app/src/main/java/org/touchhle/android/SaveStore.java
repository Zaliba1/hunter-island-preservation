/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.content.Context;
import android.net.Uri;
import org.json.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.nio.file.attribute.FileTime;
import java.security.MessageDigest;
import java.time.Instant;
import java.util.*;
import java.util.stream.Stream;
import java.util.zip.*;

/** Private, bounded, checkpoint-only save transfer compatible with the PC tool.
 * No IPA, running CPU/RAM snapshot, original URI or automatic upload is exported.
 */
final class SaveStore {
    static final String SCHEMA="hunter-game-save-debug-v1";
    static final long MAX_FILE=128L*1024*1024,MAX_TOTAL=512L*1024*1024;
    private final Context context;
    private final Path parent,root,backups,journal;
    SaveStore(Context context) throws IOException {
        this(context, false);
    }
    SaveStore(Context context, boolean selectedProfile) throws IOException {
        this.context=context;
        File external=context.getExternalFilesDir(null);
        if(external==null)throw new IOException("Spielstand-Speicher nicht verfügbar");
        String profile=selectedProfile?ModOptions.selectedProfile(context):IpaInspector.BUNDLE;
        boolean testing=!profile.equals(IpaInspector.BUNDLE);
        parent=external.toPath().resolve("touchHLE_sandbox");root=parent.resolve(profile);
        backups=context.getFilesDir().toPath().resolve("save-backups");
        journal=context.getFilesDir().toPath().resolve(testing?"test-save-import-journal.json":"save-import-journal.json");
    }
    private static void stopped() throws IOException {
        if(MainActivity.gameThreadRunning())throw new IOException("Bitte das Spiel zuerst vollständig beenden und den Starter erneut öffnen.");
    }
    private static String sha(byte[] data) throws Exception {return hex(MessageDigest.getInstance("SHA-256").digest(data));}
    private static String hex(byte[] data){StringBuilder out=new StringBuilder();for(byte b:data)out.append(String.format(Locale.ROOT,"%02x",b&255));return out.toString();}
    private static byte[] bounded(InputStream in,long maximum) throws IOException {
        ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] b=new byte[16384];int n;long total=0;
        while((n=in.read(b))!=-1){total+=n;if(total>maximum)throw new IOException("Datei zu groß");out.write(b,0,n);}return out.toByteArray();
    }
    private static String name(String name) throws IOException {
        if(name.isEmpty() || name.startsWith("/") || name.contains("\\") || name.contains(":"))throw new IOException("Unsicherer Sicherungspfad");
        if(name.toLowerCase(Locale.ROOT).endsWith(".ipa"))throw new IOException("Eine IPA ist kein Spielstand");
        for(String part:name.split("/",-1)) {
            String base=part.split("\\.",2)[0].toUpperCase(Locale.ROOT);
            if(part.isEmpty() || part.equals(".") || part.equals("..") || part.length()>255 || part.endsWith(" ") || part.endsWith(".")
                || base.matches("CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9]"))throw new IOException("Ungültiger Sicherungspfad");
            for(char c:part.toCharArray())if(c<32 || "<>\"|?*".indexOf(c)>=0)throw new IOException("Ungültiger Dateiname");
        }
        return name;
    }
    private static JSONObject record(Path file) throws Exception {
        long before=Files.size(file);FileTime stamp=Files.getLastModifiedTime(file);
        if(before>MAX_FILE || !Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS))throw new IOException("Ungültige Spielstanddatei");
        MessageDigest digest=MessageDigest.getInstance("SHA-256");byte[] buffer=new byte[16384];long bytes=0;int n;
        try(InputStream in=Files.newInputStream(file)){while((n=in.read(buffer))!=-1){bytes+=n;if(bytes>MAX_FILE)throw new IOException("Datei zu groß");digest.update(buffer,0,n);}}
        if(before!=bytes || before!=Files.size(file) || !stamp.equals(Files.getLastModifiedTime(file)))throw new IOException("Spielstand wurde während der Sicherung verändert");
        return new JSONObject().put("bytes",bytes).put("sha256",hex(digest.digest())).put("mtime_ns",stamp.toMillis()*1000000L);
    }
    private File snapshot(boolean allowEmpty,String reason) throws Exception {
        stopped();JSONObject records=new JSONObject();TreeMap<String,Path> files=new TreeMap<>();long total=0;
        if(Files.exists(root,LinkOption.NOFOLLOW_LINKS)) {
            if(Files.isSymbolicLink(root))throw new IOException("Spielstand darf kein Link sein");
            try(Stream<Path> stream=Files.walk(root)) {
                Iterator<Path> iterator=stream.iterator();
                while(iterator.hasNext()) {
                    Path p=iterator.next();if(Files.isSymbolicLink(p))throw new IOException("Links werden nicht gesichert");
                    if(Files.isDirectory(p,LinkOption.NOFOLLOW_LINKS))continue;
                    String relative=name("guest-data/"+root.relativize(p).toString().replace(File.separatorChar,'/'));
                    JSONObject info=record(p);total+=info.getLong("bytes");
                    if(total>MAX_TOTAL || files.size()>=9999)throw new IOException("Sicherung zu groß");records.put(relative,info);files.put(relative,p);
                }
            }
        }
        if(files.isEmpty() && !allowEmpty)throw new IOException("Noch keine Spielstanddaten vorhanden");
        JSONObject manifest=new JSONObject().put("schema",SCHEMA).put("bundle",IpaInspector.BUNDLE).put("kind","game-save-and-diagnostics")
            .put("emulator_savestate",false).put("created_utc",Instant.now().toString()).put("producer","Hunter Island Android "+BuildConfig.VERSION_NAME)
            .put("files",records).put("save_files",new JSONArray(files.keySet())).put("limits","Last game-written checkpoint only; no CPU, RAM, graphics or audio state.");
        Files.createDirectories(backups);File temp=File.createTempFile("save-",".partial",backups.toFile());
        try {
            try(ZipOutputStream zip=new ZipOutputStream(new FileOutputStream(temp))) {
                zip.putNextEntry(new ZipEntry("manifest.json"));zip.write(manifest.toString(2).getBytes(StandardCharsets.UTF_8));zip.closeEntry();
                for(Map.Entry<String,Path> entry:files.entrySet()) {
                    zip.putNextEntry(new ZipEntry(entry.getKey()));Files.copy(entry.getValue(),zip);zip.closeEntry();
                    if(!record(entry.getValue()).getString("sha256").equals(records.getJSONObject(entry.getKey()).getString("sha256")))throw new IOException("Spielstand wurde während der Sicherung verändert");
                }
            }
            stopped();Path target=backups.resolve("HunterIsland-"+reason+"-"+Instant.now().toEpochMilli()+"-"+UUID.randomUUID()+".zip");
            Files.move(temp.toPath(),target,StandardCopyOption.ATOMIC_MOVE);return target.toFile();
        }finally{Files.deleteIfExists(temp.toPath());}
    }
    private void copyToUri(File file,Uri uri) throws Exception {
        try(OutputStream out=context.getContentResolver().openOutputStream(uri,"wt");InputStream in=new FileInputStream(file)) {
            if(out==null)throw new IOException("Exportziel nicht verfügbar");byte[] b=new byte[16384];int n;while((n=in.read(b))!=-1)out.write(b,0,n);
        }
    }
    void exportCurrent(Uri uri) throws Exception {
        new GameStore(context).exclusive(()->{recoverLocked();File saved=snapshot(false,"manual");copyToUri(saved,uri);return null;});
    }
    void exportLatestBackup(Uri uri) throws Exception {
        new GameStore(context).exclusive(()->{stopped();File latest=latestBackup();if(latest==null)throw new IOException("Noch keine lokale Sicherung vorhanden");copyToUri(latest,uri);return null;});
    }
    File latestBackup() {
        File[] candidates=backups.toFile().listFiles((dir,n)->n.startsWith("HunterIsland-before-import-") && n.endsWith(".zip"));
        return candidates==null?null:Arrays.stream(candidates).max(Comparator.comparing(File::getName)).orElse(null);
    }
    private void journal(Path old,Path stage) throws Exception {
        byte[] bytes=new JSONObject().put("old",old.getFileName().toString()).put("stage",stage.getFileName().toString()).put("had_original",Files.exists(root)).toString().getBytes(StandardCharsets.UTF_8);
        Path temp=journal.resolveSibling("save-import-journal.partial");
        try(FileOutputStream out=new FileOutputStream(temp.toFile())){out.write(bytes);out.getFD().sync();}
        Files.move(temp,journal,StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);
    }
    private Path ownedSibling(String value,String prefix) throws IOException {
        if(!value.matches(java.util.regex.Pattern.quote(prefix)+"[0-9a-f-]{36}"))throw new IOException("Wiederherstellungsprotokoll beschädigt");return parent.resolve(value);
    }
    // Caller owns GameStore's exclusive lock. Never resume a half-committed import.
    void recoverLocked() throws Exception {
        if(!Files.exists(journal))return;stopped();
        if(Files.isSymbolicLink(journal) || Files.size(journal)>4096)throw new IOException("Wiederherstellungsprotokoll beschädigt");
        JSONObject state=new JSONObject(new String(Files.readAllBytes(journal),StandardCharsets.UTF_8));
        Path old=ownedSibling(state.getString("old"),".hunter-old-"),stage=ownedSibling(state.getString("stage"),".hunter-stage-");
        if(!Files.exists(root) && Files.exists(old))Files.move(old,root,StandardCopyOption.ATOMIC_MOVE);
        if(!Files.exists(root) && !state.optBoolean("had_original",true))Files.createDirectories(root);
        if(!Files.exists(root))throw new IOException("Unterbrochener Import: letzte Sicherung bitte wiederherstellen");
        removeStage(stage);Files.delete(journal);
    }
    private void removeStage(Path stage) throws IOException {
        if(!stage.getParent().equals(parent) || !stage.getFileName().toString().startsWith(".hunter-stage-"))throw new IOException("Ungültiges Stagingziel");
        if(Files.exists(stage,LinkOption.NOFOLLOW_LINKS))try(Stream<Path> stream=Files.walk(stage)){for(Path p:stream.sorted(Comparator.reverseOrder()).toArray(Path[]::new))Files.delete(p);}
    }
    int importSave(Uri uri) throws Exception {
        return new GameStore(context).exclusive(()->importLocked(uri));
    }
    private int importLocked(Uri uri) throws Exception {
        stopped();recoverLocked();Files.createDirectories(parent);Path stage=parent.resolve(".hunter-stage-"+UUID.randomUUID());Files.createDirectory(stage);
        File archive=File.createTempFile("save-import-",".zip",context.getCacheDir());
        try {
            try(InputStream in=context.getContentResolver().openInputStream(uri);OutputStream out=new FileOutputStream(archive)) {
                if(in==null)throw new IOException("Sicherung nicht lesbar");byte[] b=new byte[16384];int n;long size=0;
                while((n=in.read(b))!=-1){size+=n;if(size>MAX_TOTAL)throw new IOException("Sicherung zu groß");out.write(b,0,n);}
            }
            JSONObject manifest=null;Map<String,JSONObject> actual=new LinkedHashMap<>();Set<String> names=new HashSet<>();long total=0;int count=0,guestFiles=0;
            try(ZipInputStream zip=new ZipInputStream(new FileInputStream(archive))) {
                ZipEntry entry;while((entry=zip.getNextEntry())!=null) {
                    String path=name(entry.getName());if(entry.isDirectory() || !names.add(path.toLowerCase(Locale.ROOT)) || ++count>10000)throw new IOException("Doppelte/ungültige ZIP-Einträge");
                    if(path.equals("manifest.json")){manifest=new JSONObject(new String(bounded(zip,1024*1024),StandardCharsets.UTF_8));continue;}
                    boolean guest=path.startsWith("guest-data/");if(!guest && !path.startsWith("diagnostics/"))throw new IOException("Unbekannter Paketinhalt");
                    MessageDigest digest=MessageDigest.getInstance("SHA-256");long size=0;int n;byte[] b=new byte[16384];
                    Path dest=guest?stage.resolve(path.substring("guest-data/".length())):null;
                    if(dest!=null){if(!dest.normalize().startsWith(stage))throw new IOException("Unsicherer Zielpfad");Files.createDirectories(dest.getParent());guestFiles++;}
                    try(OutputStream out=dest==null?new OutputStream(){public void write(int b){}public void write(byte[] b,int o,int n){}}:Files.newOutputStream(dest,StandardOpenOption.CREATE_NEW)) {
                        while((n=zip.read(b))!=-1){size+=n;total+=n;if(size>MAX_FILE || total>MAX_TOTAL)throw new IOException("Sicherung zu groß");digest.update(b,0,n);out.write(b,0,n);}
                    }
                    actual.put(path,new JSONObject().put("bytes",size).put("sha256",hex(digest.digest())));
                }
            }
            if(manifest==null || !SCHEMA.equals(manifest.optString("schema")) || !IpaInspector.BUNDLE.equals(manifest.optString("bundle")) || !Boolean.FALSE.equals(manifest.opt("emulator_savestate")) || guestFiles==0)throw new IOException("Keine kompatible Hunter-Island-Sicherung");
            JSONObject expected=manifest.getJSONObject("files");if(expected.length()!=actual.size())throw new IOException("Manifest stimmt nicht mit ZIP überein");
            for(Map.Entry<String,JSONObject> entry:actual.entrySet()) {
                JSONObject want=expected.getJSONObject(entry.getKey()),got=entry.getValue();Object bytes=want.get("bytes");
                if(!(bytes instanceof Integer || bytes instanceof Long) || ((Number)bytes).longValue()!=got.getLong("bytes") || !want.getString("sha256").equals(got.getString("sha256")))throw new IOException("Prüfsumme stimmt nicht");
                if(entry.getKey().startsWith("guest-data/") && want.has("mtime_ns")) {
                    Object time=want.get("mtime_ns");if(!(time instanceof Integer || time instanceof Long) || ((Number)time).longValue()<0)throw new IOException("Ungültiger Zeitstempel");
                    Files.setLastModifiedTime(stage.resolve(entry.getKey().substring(11)),FileTime.fromMillis(((Number)time).longValue()/1000000));
                }
            }
            stopped();snapshot(true,"before-import"); // Original data is backed up before any rename.
            Path old=parent.resolve(".hunter-old-"+UUID.randomUUID());journal(old,stage);boolean moved=false;
            try {
                if(Files.exists(root)){Files.move(root,old,StandardCopyOption.ATOMIC_MOVE);moved=true;}
                Files.move(stage,root,StandardCopyOption.ATOMIC_MOVE);Files.delete(journal);
            }catch(Exception e){if(moved && !Files.exists(root))Files.move(old,root,StandardCopyOption.ATOMIC_MOVE);throw e;}
            return guestFiles;
        }finally{Files.deleteIfExists(archive.toPath());if(!Files.exists(journal))removeStage(stage);}
    }
}
