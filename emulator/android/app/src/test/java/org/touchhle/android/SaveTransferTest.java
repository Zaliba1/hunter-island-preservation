/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import static org.junit.Assert.*;
import android.content.Context;
import android.net.Uri;
import org.json.*;
import org.junit.*;
import org.junit.runner.RunWith;
import org.robolectric.*;
import org.robolectric.annotation.Config;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.concurrent.CountDownLatch;
import java.util.zip.*;

@RunWith(RobolectricTestRunner.class)
@Config(sdk={26,35})
public class SaveTransferTest {
    private Context context;private Path root;
    @Before public void setup(){context=RuntimeEnvironment.getApplication();root=context.getExternalFilesDir(null).toPath().resolve("touchHLE_sandbox").resolve(IpaInspector.BUNDLE);}
    private static String hash(byte[] bytes)throws Exception{StringBuilder s=new StringBuilder();for(byte b:MessageDigest.getInstance("SHA-256").digest(bytes))s.append(String.format(Locale.ROOT,"%02x",b&255));return s.toString();}
    private static byte[] pack(Map<String,byte[]> files,String bundle,boolean corrupt)throws Exception {
        JSONObject records=new JSONObject();for(Map.Entry<String,byte[]> e:files.entrySet())records.put(e.getKey(),new JSONObject().put("bytes",e.getValue().length).put("sha256",corrupt?"0".repeat(64):hash(e.getValue())));
        JSONObject m=new JSONObject().put("schema",SaveStore.SCHEMA).put("bundle",bundle).put("emulator_savestate",false).put("files",records);
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();try(ZipOutputStream z=new ZipOutputStream(bytes)){
            z.putNextEntry(new ZipEntry("manifest.json"));z.write(m.toString().getBytes(StandardCharsets.UTF_8));z.closeEntry();
            for(Map.Entry<String,byte[]> e:files.entrySet()){z.putNextEntry(new ZipEntry(e.getKey()));z.write(e.getValue());z.closeEntry();}
        }return bytes.toByteArray();
    }
    private Uri register(byte[] bytes){Uri u=Uri.parse("content://private-test/save-"+UUID.randomUUID()+".zip");Shadows.shadowOf(context.getContentResolver()).registerInputStream(u,new ByteArrayInputStream(bytes));return u;}
    private Path original()throws Exception{Path p=root.resolve("Documents/saves0/sv_1");Files.createDirectories(p.getParent());Files.write(p,"phone-original".getBytes(StandardCharsets.UTF_8));return p;}
    private byte[] exported(SaveStore store,boolean previous)throws Exception {
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();Uri u=Uri.parse("content://private-test/export-"+UUID.randomUUID()+".zip");
        Shadows.shadowOf(context.getContentResolver()).registerOutputStream(u,bytes);
        if(previous)store.exportLatestBackup(u);else store.exportCurrent(u);return bytes.toByteArray();
    }
    private Map<String,byte[]> contents(byte[] bytes)throws Exception {
        Map<String,byte[]> data=new HashMap<>();try(ZipInputStream z=new ZipInputStream(new ByteArrayInputStream(bytes))){ZipEntry e;while((e=z.getNextEntry())!=null){ByteArrayOutputStream b=new ByteArrayOutputStream();byte[] buf=new byte[1024];int n;while((n=z.read(buf))!=-1)b.write(buf,0,n);data.put(e.getName(),b.toByteArray());}}return data;
    }
    @Test public void pcImportPreservesPreviousAndExportsPortableCheckpoint()throws Exception {
        original();SaveStore store=new SaveStore(context);Map<String,byte[]> pc=new LinkedHashMap<>();pc.put("guest-data/Documents/saves0/sv_2","pc-checkpoint".getBytes(StandardCharsets.UTF_8));pc.put("diagnostics/note.txt","ignored".getBytes(StandardCharsets.UTF_8));
        assertEquals(1,store.importSave(register(pack(pc,IpaInspector.BUNDLE,false))));assertFalse(Files.exists(root.resolve("Documents/saves0/sv_1")));
        assertEquals("pc-checkpoint",new String(Files.readAllBytes(root.resolve("Documents/saves0/sv_2")),StandardCharsets.UTF_8));
        Map<String,byte[]> before=contents(exported(store,true));assertEquals("phone-original",new String(before.get("guest-data/Documents/saves0/sv_1"),StandardCharsets.UTF_8));
        String backup=store.latestBackup().getName();Map<String,byte[]> after=contents(exported(store,false));assertTrue(after.containsKey("guest-data/Documents/saves0/sv_2"));assertEquals(backup,store.latestBackup().getName());
        JSONObject m=new JSONObject(new String(after.get("manifest.json"),StandardCharsets.UTF_8));assertEquals(SaveStore.SCHEMA,m.getString("schema"));assertFalse(m.getBoolean("emulator_savestate"));assertFalse(m.toString().contains("content://"));assertEquals(1,m.getJSONObject("files").length());
    }
    @Test public void invalidHashAndWrongBundleKeepCurrentCheckpoint()throws Exception {
        Path p=original();SaveStore store=new SaveStore(context);Map<String,byte[]> pc=Collections.singletonMap("guest-data/Documents/saves0/sv_2",new byte[]{1});
        for(byte[] bad:new byte[][]{pack(pc,IpaInspector.BUNDLE,true),pack(pc,"other.game",false),pack(Collections.singletonMap("diagnostics/note.txt",new byte[]{1}),IpaInspector.BUNDLE,false)}){
            try{store.importSave(register(bad));fail("Invalid package accepted");}catch(Exception expected){}
            assertEquals("phone-original",new String(Files.readAllBytes(p),StandardCharsets.UTF_8));assertNull(store.latestBackup());
        }
    }
    @Test public void traversalCaseDuplicatesAndGameCopiesAreRejected()throws Exception {
        Path p=original();SaveStore store=new SaveStore(context);
        for(Map<String,byte[]> bad:Arrays.asList(Collections.singletonMap("guest-data/../../outside",new byte[]{1}),Collections.singletonMap("guest-data/game.ipa",new byte[]{1}),new LinkedHashMap<String,byte[]>(){{put("guest-data/Documents/X",new byte[]{1});put("guest-data/documents/x",new byte[]{2});}})){
            try{store.importSave(register(pack(bad,IpaInspector.BUNDLE,false)));fail("Unsafe package accepted");}catch(Exception expected){}
            assertEquals("phone-original",new String(Files.readAllBytes(p),StandardCharsets.UTF_8));
        }
    }
    @Test public void truncatedArchiveDoesNotReplaceCurrentCheckpoint()throws Exception {
        Path p=original();byte[] valid=pack(Collections.singletonMap("guest-data/Documents/data",new byte[1024]),IpaInspector.BUNDLE,false);
        try{new SaveStore(context).importSave(register(Arrays.copyOf(valid,valid.length/2)));fail("Truncated archive accepted");}catch(Exception expected){}
        assertEquals("phone-original",new String(Files.readAllBytes(p),StandardCharsets.UTF_8));
    }
    @Test public void interruptedRenameIsRecoveredBeforeGameLaunch()throws Exception {
        original();GameStore game=new GameStore(context);game.importGame(register(PreservationFrameworkTest.fixture(IpaInspector.BUNDLE)));
        Path old=root.resolveSibling(".hunter-old-"+UUID.randomUUID()),stage=root.resolveSibling(".hunter-stage-"+UUID.randomUUID());Files.move(root,old);Files.createDirectories(stage);Files.write(stage.resolve("new-data"),new byte[]{2});
        JSONObject j=new JSONObject().put("old",old.getFileName().toString()).put("stage",stage.getFileName().toString()).put("had_original",true);
        Files.write(context.getFilesDir().toPath().resolve("save-import-journal.json"),j.toString().getBytes(StandardCharsets.UTF_8));
        assertTrue(game.game().isFile());assertEquals("phone-original",new String(Files.readAllBytes(root.resolve("Documents/saves0/sv_1")),StandardCharsets.UTF_8));assertFalse(Files.exists(stage));
    }
    @Test public void interruptedFirstImportRollsBackToEmptyData()throws Exception {
        Path old=root.resolveSibling(".hunter-old-"+UUID.randomUUID()),stage=root.resolveSibling(".hunter-stage-"+UUID.randomUUID());Files.createDirectories(stage);Files.write(stage.resolve("new-data"),new byte[]{2});
        JSONObject j=new JSONObject().put("old",old.getFileName().toString()).put("stage",stage.getFileName().toString()).put("had_original",false);
        Files.write(context.getFilesDir().toPath().resolve("save-import-journal.json"),j.toString().getBytes(StandardCharsets.UTF_8));
        new SaveStore(context).recoverLocked();assertTrue(Files.isDirectory(root));assertFalse(Files.exists(stage));
    }
    @Test public void runningNativeThreadBlocksSaveMutation()throws Exception {
        Path p=original();java.lang.reflect.Field field=org.libsdl.app.SDLActivity.class.getDeclaredField("mSDLThread");field.setAccessible(true);CountDownLatch ready=new CountDownLatch(1);
        Thread thread=new Thread(()->{ready.countDown();try{Thread.sleep(30000);}catch(InterruptedException ignored){}});thread.start();ready.await();field.set(null,thread);
        try {
            SaveStore store=new SaveStore(context);try{store.importSave(register(new byte[]{0}));fail("Active game import accepted");}catch(IOException expected){}
            try{exported(store,false);fail("Active game export accepted");}catch(IOException expected){}
            assertEquals("phone-original",new String(Files.readAllBytes(p),StandardCharsets.UTF_8));
        }finally{field.set(null,null);thread.interrupt();thread.join();}
    }
}
