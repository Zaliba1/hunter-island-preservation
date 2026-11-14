/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.app.Activity;
import android.app.AlertDialog;
import android.os.Bundle;
import android.text.Html;
import android.widget.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Offline access to the actual notices bundled in this APK. */
public final class LicenseActivity extends Activity {
    static final String ROOT="preservation-notices/";
    private static final int LIMIT=16*1024*1024;
    static String readNotice(android.content.res.AssetManager assets,String relative) throws IOException {
        if(relative.isEmpty() || relative.startsWith("/") || relative.contains("\\")
                || Arrays.asList(relative.split("/",-1)).contains(".."))throw new IOException("Ungültiger Hinweispfad");
        try(InputStream in=assets.open(ROOT+relative);ByteArrayOutputStream out=new ByteArrayOutputStream()) {
            byte[] buffer=new byte[8192];int n;
            while((n=in.read(buffer))!=-1){if(out.size()+n>LIMIT)throw new IOException("Hinweis zu groß");out.write(buffer,0,n);}
            return new String(out.toByteArray(),StandardCharsets.UTF_8);
        }
    }
    private void listNotices(String directory,List<String> result) throws IOException {
        String[] entries=getAssets().list(directory.isEmpty()?ROOT.substring(0,ROOT.length()-1):ROOT+directory);
        if(entries==null)return;
        Arrays.sort(entries);
        for(String name:entries) {
            String relative=directory.isEmpty()?name:directory+"/"+name;
            String[] children=getAssets().list(ROOT+relative);
            if(children!=null && children.length>0)listNotices(relative,result);else result.add(relative);
        }
    }
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root=new LinearLayout(this);root.setOrientation(LinearLayout.VERTICAL);
        int pad=(int)(16*getResources().getDisplayMetrics().density);
        TextView title=new TextView(this);title.setText("Lizenzen und Quellen");title.setTextSize(24);title.setPadding(pad,pad,pad,pad);root.addView(title);
        Button back=new Button(this);back.setText("Zurück zum Starter");back.setOnClickListener(v->finish());root.addView(back);
        ListView list=new ListView(this);root.addView(list,new LinearLayout.LayoutParams(-1,0,1));setContentView(root);
        try {
            ArrayList<String> files=new ArrayList<>();listNotices("",files);
            // Put the source-delivery and license summaries before individual texts.
            files.remove("SOURCE.md");files.remove("LICENSES.md");files.add(0,"SOURCE.md");files.add(0,"LICENSES.md");
            list.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_list_item_1,files));
            list.setOnItemClickListener((parent,view,position,id)->show(files.get(position),pad));
        }catch(IOException e){failure(e);}
    }
    private void show(String name,int pad) {
        try {
            String content=readNotice(getAssets(),name);TextView text=new TextView(this);text.setPadding(pad,pad,pad,pad);text.setTextIsSelectable(true);
            text.setText(name.endsWith(".html")?Html.fromHtml(content,Html.FROM_HTML_MODE_LEGACY):content);
            ScrollView scroll=new ScrollView(this);scroll.addView(text);
            new AlertDialog.Builder(this).setTitle(name).setView(scroll).setPositiveButton("Schließen",null).show();
        }catch(IOException e){failure(e);}
    }
    private void failure(IOException e){new AlertDialog.Builder(this).setTitle("Hinweis nicht verfügbar").setMessage(e.getMessage()).setPositiveButton("OK",null).show();}
}
