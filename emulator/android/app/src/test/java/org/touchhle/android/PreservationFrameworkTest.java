/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import static org.junit.Assert.*;
import android.app.Application;
import android.content.Context;
import android.content.ComponentName;
import android.content.pm.ApplicationInfo;
import android.net.Uri;
import android.widget.Button;
import android.view.View;
import android.view.ViewGroup;
import org.junit.*;
import org.junit.runner.RunWith;
import org.robolectric.*;
import org.robolectric.android.controller.ActivityController;
import org.robolectric.annotation.Config;
import java.io.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;
import java.util.zip.*;

@RunWith(RobolectricTestRunner.class)
@Config(sdk={26,35})
public class PreservationFrameworkTest {
    private Context context;
    @Before public void setup(){context=RuntimeEnvironment.getApplication();}
    static byte[] fixture(String bundle) throws Exception {
        ByteBuffer executable=ByteBuffer.allocate(48).order(ByteOrder.LITTLE_ENDIAN);
        for(int n:new int[]{0xfeedface,12,6,2,1,20,0,0x21,20,0,0,0})executable.putInt(n);
        String plist="<?xml version=\"1.0\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict>"
            +"<key>CFBundleIdentifier</key><string>"+bundle+"</string><key>CFBundleExecutable</key><string>Game</string>"
            +"<key>CFBundleShortVersionString</key><string>2.02</string><key>MinimumOSVersion</key><string>6.0</string></dict></plist>";
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(bytes)) {
            zip.putNextEntry(new ZipEntry("Payload/Game.app/Info.plist"));zip.write(plist.getBytes(StandardCharsets.UTF_8));zip.closeEntry();
            zip.putNextEntry(new ZipEntry("Payload/Game.app/Game"));zip.write(executable.array());zip.closeEntry();
        }
        return bytes.toByteArray();
    }
    private Uri register(byte[] data) {
        Uri uri=Uri.parse("content://test.fixture/private-original-"+UUID.randomUUID()+".ipa");
        Shadows.shadowOf(context.getContentResolver()).registerInputStream(uri,new ByteArrayInputStream(data));return uri;
    }
    private File archive(byte[] data) throws Exception {
        File path=File.createTempFile("inspector-",".ipa",context.getCacheDir());Files.write(path.toPath(),data);return path;
    }
    @Test public void androidXmlParserAcceptsStandardApplePlist() throws Exception {
        IpaInspector.Info info=IpaInspector.inspect(archive(fixture(IpaInspector.BUNDLE)));
        assertEquals("2.02",info.version);assertEquals("6.0",info.minimumIOS);
    }
    @Test public void rejectedImportKeepsPreviousGameAndMetadata() throws Exception {
        GameStore store=new GameStore(context);store.importGame(register(fixture(IpaInspector.BUNDLE)));
        String before=store.settings().toString();File game=store.game();String hash=IpaInspector.digest(game);
        try {store.importGame(register(fixture("other.game")));fail("Wrong game accepted");} catch(IOException expected) {}
        assertEquals(before,store.settings().toString());assertEquals(hash,IpaInspector.digest(store.game()));
        assertEquals(0,Objects.requireNonNull(new File(context.getFilesDir(),"games").listFiles((d,n)->n.endsWith(".partial"))).length);
    }
    @Test public void importDoesNotRetainOriginalUri() throws Exception {
        GameStore store=new GameStore(context);Uri uri=register(fixture(IpaInspector.BUNDLE));store.importGame(uri);
        String json=store.settings().toString();assertFalse(json.contains("content://"));assertFalse(json.contains("private-original"));
    }
    @Test public void modifiedPrivateCopyIsRejectedBeforeLaunch() throws Exception {
        GameStore store=new GameStore(context);store.importGame(register(fixture(IpaInspector.BUNDLE)));
        Files.write(store.game().toPath(),new byte[]{1,2,3});
        try {store.game();fail("Modified IPA accepted");}catch(IOException expected){}
    }
    @Test public void parallelImportIsRejectedWhilePrivateLockIsHeld() throws Exception {
        GameStore store=new GameStore(context);
        try(RandomAccessFile file=new RandomAccessFile(new File(context.getFilesDir(),"import.lock"),"rw");java.nio.channels.FileLock lock=file.getChannel().lock()) {
            try {store.importGame(register(fixture(IpaInspector.BUNDLE)));fail("Concurrent import accepted");}catch(IOException expected){}
        }
        assertFalse(store.settings().has("sha256"));
    }
    @Test public void diagnosticMetadataUsesAnExplicitAllowlist() throws Exception {
        GameStore store=new GameStore(context);store.importGame(register(fixture(IpaInspector.BUNDLE)));
        org.json.JSONObject value=store.settings();value.put("original_uri","content://private-player/secret.ipa");
        Files.write(new File(context.getFilesDir(),"import.json").toPath(),value.toString().getBytes(StandardCharsets.UTF_8));
        assertFalse(store.diagnosticGame().toString().contains("private-player"));assertEquals(4,store.diagnosticGame().length());
    }
    private static Button button(View view,String label) {
        if(view instanceof Button && label.contentEquals(((Button)view).getText()))return (Button)view;
        if(view instanceof ViewGroup)for(int i=0;i<((ViewGroup)view).getChildCount();i++){Button b=button(((ViewGroup)view).getChildAt(i),label);if(b!=null)return b;}
        return null;
    }
    @Test public void launcherModsDefaultOffAndReachTheGameIndependently() {
        String[] labels = {
            "Find rare starter dragons more often (5% per wild battle)",
            "Exchange silver for gold (50:1)",
            "Skip the 12-hour arena wait",
            "Full collection and resources (separate save copy)",
            "New game with all monsters at level 1",
            "New game with 3 of each monster, level 99, rank S",
        };
        String[] flags = {
            "--hunter-rare-dragons", "--hunter-silver-exchange", "--hunter-no-arena-cooldown",
            "--hunter-test-save",
            "--hunter-level1-save",
            "--hunter-triple99-save",
        };
        try (ActivityController<LauncherActivity> controller =
                Robolectric.buildActivity(LauncherActivity.class).setup()) {
            LauncherActivity activity = controller.get();
            android.widget.CheckBox[] boxes = new android.widget.CheckBox[labels.length];
            for (int i = 0; i < labels.length; i++) {
                boxes[i] = (android.widget.CheckBox) button(activity.getWindow().getDecorView(), labels[i]);
                assertNotNull(boxes[i]);
                assertFalse(boxes[i].isChecked());
            }
            for (int mask = 0; mask < (1 << flags.length); mask++) {
                List<String> expected = new ArrayList<>();
                for (int i = 0; i < flags.length; i++) {
                    boolean enabled = (mask & (1 << i)) != 0;
                    boxes[i].setChecked(enabled);
                }
                for (int i=0;i<flags.length;i++) if(boxes[i].isChecked()) expected.add(flags[i]);
                assertTrue((boxes[3].isChecked()?1:0)+(boxes[4].isChecked()?1:0)+(boxes[5].isChecked()?1:0)<=1);
                List<String> arguments = new ArrayList<>();
                ModOptions.appendArguments(activity, arguments);
                assertEquals(expected, arguments);
            }
            controller.recreate();
            for (String label : labels) {
                android.widget.CheckBox box = (android.widget.CheckBox)
                    button(controller.get().getWindow().getDecorView(), label);
                assertNotNull(box);
                assertEquals(!label.equals(labels[3]) && !label.equals(labels[4]),box.isChecked());
                box.setChecked(false);
            }
        }
    }
    @Test public void levelOneProfileNeedsNoNormalSaveAndConflictingProfilesAreRejected() throws Exception {
        android.content.SharedPreferences p=context.getSharedPreferences("mods",Context.MODE_PRIVATE);
        p.edit().putBoolean("--hunter-level1-save",true).putBoolean("--hunter-rare-dragons",true).commit();
        ModOptions.requireTestingSeed(context);
        assertEquals("pockettrend.hunterisland-level1-v1",ModOptions.selectedProfile(context));
        List<String> arguments=new ArrayList<>();ModOptions.appendArguments(context,arguments);
        assertEquals(Arrays.asList("--hunter-rare-dragons","--hunter-level1-save"),arguments);
        p.edit().putBoolean("--hunter-test-save",true).commit();
        try {ModOptions.selectedProfile(context);fail("Conflicting testing profiles accepted");}catch(IllegalStateException expected){}
    }
    @Test public void triple99NeedsNoSeedPersistsAndRejectsEveryOtherCollection() throws Exception {
        android.content.SharedPreferences p=context.getSharedPreferences("mods",Context.MODE_PRIVATE);
        p.edit().putBoolean("--hunter-triple99-save",true).commit();
        ModOptions.requireTestingSeed(context);
        assertEquals("pockettrend.hunterisland-triple99-v1",ModOptions.selectedProfile(context));
        List<String> arguments=new ArrayList<>();ModOptions.appendArguments(context,arguments);
        assertEquals(Arrays.asList("--hunter-triple99-save"),arguments);
        for(String flag:Arrays.asList("--hunter-test-save","--hunter-level1-save")) {
            p.edit().putBoolean(flag,true).commit();
            try{ModOptions.selectedProfile(context);fail("Conflicting collections accepted");}catch(IllegalStateException expected){}
            p.edit().putBoolean(flag,false).commit();
        }
    }
    @Test public void integerScalingIsOptionalPersistsAndKeepsTheExistingDisplayProfile() {
        android.content.SharedPreferences display = context.getSharedPreferences("display", Context.MODE_PRIVATE);
        display.edit().clear().putString("family", "ipad").commit();
        try (ActivityController<LauncherActivity> controller =
                Robolectric.buildActivity(LauncherActivity.class).setup()) {
            android.widget.CheckBox scaling = controller.get().getWindow().getDecorView()
                .findViewWithTag(DisplayOptions.INTEGER_SCALING_TAG);
            assertFalse(scaling.isChecked());
            List<String> arguments = new ArrayList<>();
            DisplayOptions.appendArguments(controller.get(), arguments);
            assertEquals(Arrays.asList("--device-family=ipad"), arguments);

            scaling.performClick();
            controller.recreate();
            scaling = controller.get().getWindow().getDecorView().findViewWithTag(DisplayOptions.INTEGER_SCALING_TAG);
            assertTrue(scaling.isChecked());
            arguments.clear();
            DisplayOptions.appendArguments(controller.get(), arguments);
            assertEquals(Arrays.asList("--device-family=ipad", "--integer-scaling"), arguments);

            scaling.performClick();
            display.edit().putString("family", "iphone-retina").commit();
            arguments.clear();
            DisplayOptions.appendArguments(controller.get(), arguments);
            assertEquals(Arrays.asList("--device-family=iphone", "--experimental-retina"), arguments);
        }
    }

    @Test public void internalResolutionIsIndependentOfLocalTextures() {
        try (ActivityController<LauncherActivity> controller =
                Robolectric.buildActivity(LauncherActivity.class).setup()) {
            android.widget.CheckBox box = controller.get().getWindow().getDecorView().findViewWithTag("hunter-internal-2x");
            assertFalse(box.isChecked());
            box.performClick();
            controller.recreate();
            box = controller.get().getWindow().getDecorView().findViewWithTag("hunter-internal-2x");
            assertTrue(box.isChecked());
            List<String> args = new ArrayList<>();
            DisplayOptions.appendArguments(controller.get(), args);
            assertTrue(args.contains("--scale-hack=2"));
            assertFalse(args.contains("--hunter-hd-textures"));
        }
    }

    @Test public void localTextureGenerationIsOptInAndSurvivesRecreation() {
        try (ActivityController<LauncherActivity> controller =
                Robolectric.buildActivity(LauncherActivity.class).setup()) {
            android.widget.CheckBox hd = controller.get().getWindow().getDecorView().findViewWithTag("hunter-hd-textures");
            assertFalse(hd.isChecked());
            hd.performClick();
            controller.recreate();
            hd = controller.get().getWindow().getDecorView().findViewWithTag("hunter-hd-textures");
            assertTrue(hd.isChecked());
            List<String> args = new ArrayList<>();
            DisplayOptions.appendArguments(controller.get(), args);
            assertTrue(args.contains("--hunter-hd-textures"));
            hd.performClick();
            args.clear();
            DisplayOptions.appendArguments(controller.get(), args);
            assertFalse(args.contains("--hunter-hd-textures"));
        }
    }

    @Test public void launcherRefreshesImportWhenReturning() throws Exception {
        try(ActivityController<LauncherActivity> controller=Robolectric.buildActivity(LauncherActivity.class).setup()) {
            LauncherActivity activity=controller.get();Button start=button(activity.getWindow().getDecorView(),"Play");
            assertNotNull(start);assertFalse(start.isEnabled());controller.pause();
            new GameStore(activity).importGame(register(fixture(IpaInspector.BUNDLE)));
            controller.resume();assertTrue("Stale disabled Play button after returning",start.isEnabled());
        }
    }
    @Test public void apkPermissionsAndGameActivityStayPrivate() throws Exception {
        String name=context.getPackageName();assertEquals("org.hunterislandpreservation.android",name);
        assertEquals(0,context.getApplicationInfo().flags&ApplicationInfo.FLAG_ALLOW_BACKUP);
        assertFalse(context.getPackageManager().getActivityInfo(new ComponentName(context,MainActivity.class),0).exported);
        assertFalse(context.getPackageManager().getActivityInfo(new ComponentName(context,LicenseActivity.class),0).exported);
        String[] requested=context.getPackageManager().getPackageInfo(name,android.content.pm.PackageManager.GET_PERMISSIONS).requestedPermissions;
        assertTrue(requested==null || !Arrays.asList(requested).contains("android.permission.INTERNET"));
    }
    @Test public void launcherOpensOfflineLicenseBrowser() {
        try(ActivityController<LauncherActivity> controller=Robolectric.buildActivity(LauncherActivity.class).setup()) {
            LauncherActivity activity=controller.get();
            button(activity.getWindow().getDecorView(),"Licenses and source information").performClick();
            assertEquals(LicenseActivity.class.getName(),Shadows.shadowOf(activity).getNextStartedActivity().getComponent().getClassName());
        }
    }
    @Test public void completeNoticesAndSourceDeliveryAreAccessibleOffline() throws Exception {
        assertTrue(LicenseActivity.readNotice(context.getAssets(),"licenses/GPL-3.0.txt").contains("Version 3, 29 June 2007"));
        assertTrue(LicenseActivity.readNotice(context.getAssets(),"SOURCE.md").contains("HunterIsland-Source-" + BuildConfig.VERSION_NAME + ".zip"));
        assertTrue(LicenseActivity.readNotice(context.getAssets(),"licenses/touchHLE_fonts/LICENSE.noto").contains("SIL OPEN FONT LICENSE"));
        assertTrue(LicenseActivity.readNotice(context.getAssets(),"licenses/rust-standard-library/COPYRIGHT-library.html").length()>10000);
        try{LicenseActivity.readNotice(context.getAssets(),"../touchHLE_default_options.txt");fail("Outside notice tree accepted");}catch(IOException expected){}
        try(ActivityController<LicenseActivity> controller=Robolectric.buildActivity(LicenseActivity.class).setup()) {
            ViewGroup content=controller.get().findViewById(android.R.id.content);
            ViewGroup root=(ViewGroup)content.getChildAt(0);android.widget.ListView list=(android.widget.ListView)root.getChildAt(2);
            assertTrue(list.getAdapter().getCount()>100);
            int position=-1;for(int i=0;i<list.getAdapter().getCount();i++)if("licenses/GPL-3.0.txt".equals(list.getAdapter().getItem(i)))position=i;
            assertTrue(position>=0);list.performItemClick(null,position,position);
            assertNotNull(org.robolectric.shadows.ShadowAlertDialog.getLatestAlertDialog());
        }
    }
}
