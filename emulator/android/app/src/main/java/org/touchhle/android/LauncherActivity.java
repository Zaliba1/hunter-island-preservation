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
        TextView subtitle=new TextView(this);subtitle.setText("Android-Testversion · touchHLE");root.addView(subtitle);
        TextView displayLabel=new TextView(this);displayLabel.setText("Bildformat (gilt ab dem nächsten Spielstart)");root.addView(displayLabel);
        Spinner display=new Spinner(this);
        display.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_spinner_dropdown_item,new String[]{"iPhone Retina · 960 × 640 · experimentell","iPhone · 480 × 320","iPad · 1024 × 768"}));
        android.content.SharedPreferences preferences=getSharedPreferences("display",MODE_PRIVATE);
        String family=preferences.getString("family","iphone-retina");
        display.setSelection("ipad".equals(family)?2:"iphone".equals(family)?1:0);
        display.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener(){
            public void onItemSelected(AdapterView<?> p,android.view.View v,int position,long id){preferences.edit().putString("family",position==2?"ipad":position==1?"iphone":"iphone-retina").apply();}
            public void onNothingSelected(AdapterView<?> p){}
        });root.addView(display);
        ModOptions.addControls(this, root);
        status=new TextView(this);status.setPadding(0,pad,0,pad);root.addView(status);
        choose=new Button(this);choose.setText("Eigene IPA importieren …");root.addView(choose);
        start=new Button(this);start.setText("Spiel starten");root.addView(start);
        export=new Button(this);export.setText("Diagnoseinfos exportieren …");root.addView(export);
        saveImport=new Button(this);saveImport.setText("Spielstand importieren …");root.addView(saveImport);
        saveExport=new Button(this);saveExport.setText("Spielstand exportieren …");root.addView(saveExport);
        backupExport=new Button(this);backupExport.setText("Sicherung vor Import exportieren …");root.addView(backupExport);
        Button licenses=new Button(this);licenses.setText("Hinweise und Lizenzen");root.addView(licenses);
        TextView privacy=new TextView(this);privacy.setPadding(0,pad,0,0);
        privacy.setText("Alles bleibt lokal. Deine IPA und Spielstände werden nicht mitgeliefert oder automatisch versendet.");root.addView(privacy);
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
            intent.putExtra(Intent.EXTRA_TITLE,"HunterIsland-Android-Diagnose.json");startActivityForResult(intent,EXPORT);
        });
        saveImport.setOnClickListener(v->new AlertDialog.Builder(this).setTitle("Spielstand übernehmen")
            .setMessage("Importiert eine Hunter-Island-Spielstandsicherung als ZIP. Der bisherige Handy-Stand wird vorher lokal gesichert. Es wird der letzte vom Spiel geschriebene Stand übertragen, kein Emulator-Savestate.")
            .setNegativeButton("Abbrechen",null).setPositiveButton("ZIP auswählen",(d,w)->{
                Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT);intent.setType("*/*");intent.addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,SAVE_IMPORT);
            }).show());
        saveExport.setOnClickListener(v->chooseSaveOutput(SAVE_EXPORT,"HunterIsland-Spielstand.zip"));
        backupExport.setOnClickListener(v->chooseSaveOutput(BACKUP_EXPORT,"HunterIsland-vor-Import.zip"));
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
        busy(true);status.setText("Bitte warten …");
        worker.execute(()->{try{action.run();ui(this::refresh);}catch(Exception e){ui(()->{refresh();error(e);});}});
    }
    private void busy(boolean value){working=value;for(Button button:new Button[]{choose,start,export,saveImport,saveExport,backupExport})button.setEnabled(!value);}
    private void refresh() {
        if(isFinishing() || isDestroyed())return;
        busy(false);
        try {
            JSONObject data=store.settings();boolean ready=data.has("sha256");start.setEnabled(ready);
            status.setText(ready?"Hunter Island "+data.optString("version")+" ist importiert.\nExperimentelle Android-Version.":"Importiere deine eigene, bereits entschlüsselte Hunter-Island-IPA. Bisher geprüft: Version 2.02.");
        } catch(Exception e){status.setText("Importdaten konnten nicht gelesen werden.");start.setEnabled(false);}
    }
    private void error(Exception e){if(!isFinishing())new AlertDialog.Builder(this).setTitle("Vorgang fehlgeschlagen").setMessage(e.getMessage()).setPositiveButton("OK",null).show();}
    @Override protected void onActivityResult(int request,int result,Intent intent) {
        super.onActivityResult(request,result,intent);
        if(result!=RESULT_OK || intent==null || intent.getData()==null)return;
        Uri uri=intent.getData();
        if(request==IMPORT)run(()->{
            IpaInspector.Info info=store.importGame(uri);
            if(!info.version.equals("2.02"))ui(()->new AlertDialog.Builder(this).setTitle("Ungetestete Spielversion").setMessage("Importiert: "+info.version+". Bestätigt geprüft ist bisher 2.02.").setPositiveButton("OK",null).show());
        });
        if(request==EXPORT)run(()->{
            JSONObject data=new JSONObject();data.put("schema","hunter-android-diagnostics-v1");data.put("app_version",BuildConfig.VERSION_NAME);
            data.put("android_api",android.os.Build.VERSION.SDK_INT);data.put("device_model",android.os.Build.MODEL);
            data.put("game",store.diagnosticGame());data.put("includes_saves",false);data.put("includes_raw_logs",false);
            try(OutputStream out=getContentResolver().openOutputStream(uri,"wt")) {
                if(out==null)throw new IOException("Exportziel nicht verfügbar");out.write(data.toString(2).getBytes(StandardCharsets.UTF_8));
            }
        });
        if(request==SAVE_IMPORT)run(()->{
            int files=new SaveStore(this).importSave(uri);
            ui(()->new AlertDialog.Builder(this).setTitle("Spielstand importiert").setMessage(files+" Dateien übernommen. Der vorherige Handy-Stand ist lokal gesichert und kann über ‚Sicherung vor Import exportieren‘ gesichert werden.").setPositiveButton("OK",null).show());
        });
        if(request==SAVE_EXPORT || request==BACKUP_EXPORT)run(()->{
            SaveStore saves=new SaveStore(this);if(request==SAVE_EXPORT)saves.exportCurrent(uri);else saves.exportLatestBackup(uri);
            ui(()->new AlertDialog.Builder(this).setTitle("Sicherung exportiert").setMessage("Die private ZIP-Sicherung wurde im gewählten Speicherort abgelegt. Keine IPA und kein Emulator-Savestate enthalten.").setPositiveButton("OK",null).show());
        });
    }
    @Override protected void onResume(){super.onResume();if(store!=null && !working)refresh();}
    @Override protected void onDestroy(){worker.shutdown();super.onDestroy();}
}
