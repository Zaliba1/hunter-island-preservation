/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.app.*;
import android.content.*;
import android.net.Uri;
import android.os.Bundle;
import android.widget.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;
import org.json.JSONObject;

/** Native Android front end. Imports and diagnostic exports are user-selected. */
public final class LauncherActivity extends Activity {
    private static final int IMPORT=10,EXPORT=11,SAVE_IMPORT=12,SAVE_EXPORT=13,BACKUP_EXPORT=14;
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private GameStore store;private TextView status;private Button start,choose,export,saveImport,saveExport,backupExport;private boolean working;
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root=new LinearLayout(this);root.setOrientation(LinearLayout.VERTICAL);
        int pad=(int)(24*getResources().getDisplayMetrics().density);root.setPadding(pad,pad,pad,pad);
        TextView title=new TextView(this);title.setText("Hunter Island");title.setTextSize(28);root.addView(title);
        TextView subtitle=new TextView(this);subtitle.setText("Android preview · touchHLE");root.addView(subtitle);
        TextView displayLabel=new TextView(this);displayLabel.setText("Display mode (applies on next launch)");root.addView(displayLabel);
        Spinner display=new Spinner(this);
        display.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_spinner_dropdown_item,new String[]{"iPhone Retina · 960 × 640 · experimental","iPhone · 480 × 320","iPad · 1024 × 768"}));
        android.content.SharedPreferences preferences=getSharedPreferences("display",MODE_PRIVATE);
        String family=preferences.getString("family","iphone-retina");
        display.setSelection("ipad".equals(family)?2:"iphone".equals(family)?1:0);
        display.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener(){
            public void onItemSelected(AdapterView<?> p,android.view.View v,int position,long id){preferences.edit().putString("family",position==2?"ipad":position==1?"iphone":"iphone-retina").apply();}
            public void onNothingSelected(AdapterView<?> p){}
        });root.addView(display);
        LanguageOptions.addControls(this, root);
        ModOptions.addControls(this, root);
        status=new TextView(this);status.setPadding(0,pad,0,pad);root.addView(status);
        choose=new Button(this);choose.setText("Import your IPA…");root.addView(choose);
        start=new Button(this);start.setText("Play");root.addView(start);
        export=new Button(this);export.setText("Export diagnostics…");root.addView(export);
        saveImport=new Button(this);saveImport.setText("Import save…");root.addView(saveImport);
        saveExport=new Button(this);saveExport.setText("Export save…");root.addView(saveExport);
        backupExport=new Button(this);backupExport.setText("Export pre-import backup…");root.addView(backupExport);
        Button licenses=new Button(this);licenses.setText("Licenses and source information");root.addView(licenses);
        TextView privacy=new TextView(this);privacy.setPadding(0,pad,0,0);
        privacy.setText("Your game and saves stay on this device.");root.addView(privacy);
        ScrollView page=new ScrollView(this);page.setFillViewport(true);page.setFitsSystemWindows(true);page.addView(root);setContentView(page);
        try {store=new GameStore(this);refresh();}catch(Exception e){error(e);busy(true);}
        choose.setOnClickListener(v->{
            Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT);intent.setType("*/*");intent.addCategory(Intent.CATEGORY_OPENABLE);
            startActivityForResult(intent,IMPORT);
        });
        start.setOnClickListener(v->run(()->{
            File game=store.game();
            ui(()->{Intent intent=new Intent(this,MainActivity.class);intent.putExtra("game",game.getAbsolutePath());startActivity(intent);});
        }));
        export.setOnClickListener(v->{
            Intent intent=new Intent(Intent.ACTION_CREATE_DOCUMENT);intent.setType("application/json");intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.putExtra(Intent.EXTRA_TITLE,"HunterIsland-Android-Diagnostics.json");startActivityForResult(intent,EXPORT);
        });
        saveImport.setOnClickListener(v->new AlertDialog.Builder(this).setTitle("Import save")
            .setMessage("Choose a Hunter Island save ZIP. The app backs up your current save first. The ZIP must contain an in-game save.")
            .setNegativeButton("Cancel",null).setPositiveButton("Choose ZIP…",(d,w)->{
                Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT);intent.setType("*/*");intent.addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,SAVE_IMPORT);
            }).show());
        saveExport.setOnClickListener(v->chooseSaveOutput(SAVE_EXPORT,"HunterIsland-Save.zip"));
        backupExport.setOnClickListener(v->chooseSaveOutput(BACKUP_EXPORT,"HunterIsland-Pre-Import-Backup.zip"));
        licenses.setOnClickListener(v->startActivity(new Intent(this,LicenseActivity.class)));
    }
    private interface Action {void run() throws Exception;}
    private void chooseSaveOutput(int request,String filename){
        Intent intent=new Intent(Intent.ACTION_CREATE_DOCUMENT);intent.setType("application/zip");intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.putExtra(Intent.EXTRA_TITLE,filename);startActivityForResult(intent,request);
    }
    private void ui(Runnable action){runOnUiThread(()->{if(!isDestroyed() && !isFinishing())action.run();});}
    private void run(Action action) {
        if(isDestroyed() || isFinishing())return;
        busy(true);status.setText("Please wait…");
        worker.execute(()->{try{action.run();ui(this::refresh);}catch(Exception e){ui(()->{refresh();error(e);});}});
    }
    private void busy(boolean value){working=value;for(Button button:new Button[]{choose,start,export,saveImport,saveExport,backupExport})button.setEnabled(!value);}
    private void refresh() {
        if(isFinishing() || isDestroyed())return;
        busy(false);
        try {
            JSONObject data=store.settings();boolean ready=data.has("sha256");start.setEnabled(ready);
            status.setText(ready?"Hunter Island "+data.optString("version")+" is ready to play.":"Import your own decrypted Hunter Island IPA. Version 2.02 has been tested.");
        } catch(Exception e){status.setText("Could not read the imported game settings.");start.setEnabled(false);}
    }
    private void error(Exception e){if(!isFinishing())new AlertDialog.Builder(this).setTitle("Operation failed").setMessage(e.getMessage()).setPositiveButton("OK",null).show();}
    @Override protected void onActivityResult(int request,int result,Intent intent) {
        super.onActivityResult(request,result,intent);
        if(result!=RESULT_OK || intent==null || intent.getData()==null)return;
        Uri uri=intent.getData();
        if(request==IMPORT)run(()->{
            IpaInspector.Info info=store.importGame(uri);
            if(!info.version.equals("2.02"))ui(()->new AlertDialog.Builder(this).setTitle("Untested game version").setMessage("Imported: "+info.version+". Version 2.02 is the one we have tested.").setPositiveButton("OK",null).show());
        });
        if(request==EXPORT)run(()->{
            JSONObject data=new JSONObject();data.put("schema","hunter-android-diagnostics-v1");data.put("app_version",BuildConfig.VERSION_NAME);
            data.put("android_api",android.os.Build.VERSION.SDK_INT);data.put("device_model",android.os.Build.MODEL);
            data.put("language",LanguageOptions.selectedLanguage(this));
            data.put("game",store.diagnosticGame());data.put("includes_saves",false);data.put("includes_raw_logs",false);
            try(OutputStream out=getContentResolver().openOutputStream(uri,"wt")) {
                if(out==null)throw new IOException("Could not open the export destination");out.write(data.toString(2).getBytes(StandardCharsets.UTF_8));
            }
        });
        if(request==SAVE_IMPORT)run(()->{
            int files=new SaveStore(this).importSave(uri);
            ui(()->new AlertDialog.Builder(this).setTitle("Save imported").setMessage(files+" files imported. Use Export pre-import backup to copy your previous save to another location.").setPositiveButton("OK",null).show());
        });
        if(request==SAVE_EXPORT || request==BACKUP_EXPORT)run(()->{
            SaveStore saves=new SaveStore(this,request==SAVE_EXPORT);if(request==SAVE_EXPORT)saves.exportCurrent(uri);else saves.exportLatestBackup(uri);
            ui(()->new AlertDialog.Builder(this).setTitle("Backup exported").setMessage("The save ZIP was written to the selected location.").setPositiveButton("OK",null).show());
        });
    }
    @Override protected void onResume(){super.onResume();if(store!=null && !working)refresh();}
    @Override protected void onDestroy(){worker.shutdown();super.onDestroy();}
}
