/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;
import java.util.zip.*;
import javax.xml.parsers.DocumentBuilderFactory;
import org.w3c.dom.*;
import org.xml.sax.InputSource;

/** Static, bounded ZIP/plist/Mach-O inspection. Never executes an imported file. */
public final class IpaInspector {
    public static final String BUNDLE = "pockettrend.hunterisland";
    public static final long MAX_ARCHIVE = 512L * 1024 * 1024;
    private static final long MAX_CONTENT = 1024L * 1024 * 1024;
    public static final class Info {
        public final String version, minimumIOS, sha256;
        Info(String version, String minimumIOS, String sha256) {
            this.version=version; this.minimumIOS=minimumIOS; this.sha256=sha256;
        }
    }
    private static IOException invalid(String reason) {return new IOException(reason);}
    private static void require(boolean value, String message) throws IOException {
        if (!value) throw invalid(message);
    }
    public static String digest(File file) throws Exception {
        MessageDigest md=MessageDigest.getInstance("SHA-256");
        try(InputStream in=new FileInputStream(file)) {
            byte[] buffer=new byte[65536]; int n;
            while((n=in.read(buffer))!=-1) md.update(buffer,0,n);
        }
        StringBuilder out=new StringBuilder();
        for(byte b:md.digest()) out.append(String.format(Locale.ROOT,"%02x", b&255));
        return out.toString();
    }
    private static void safeName(String name) throws IOException {
        require(!name.isEmpty() && !name.startsWith("/") && !name.contains("\\") &&
            name.indexOf('\0')<0 && !name.contains(":"), "Unsicherer Archivpfad");
        String body=name.endsWith("/")?name.substring(0,name.length()-1):name;
        for(String part:body.split("/",-1))
            require(!part.isEmpty() && !part.equals(".") && !part.equals(".."),"Unsicherer Archivpfad");
    }
    private static byte[] read(ZipFile zip, ZipEntry entry, int limit) throws IOException {
        require(entry!=null && entry.getSize()>=0 && entry.getSize()<=limit,"Datei fehlt oder ist zu gross");
        try(InputStream in=zip.getInputStream(entry);ByteArrayOutputStream out=new ByteArrayOutputStream()) {
            byte[] buffer=new byte[65536]; int n;
            while((n=in.read(buffer))!=-1) {
                require((long)out.size()+n<=limit,"Datei ueberschreitet das Limit");out.write(buffer,0,n);
            }
            return out.toByteArray();
        }
    }
    public static Info inspect(File file) throws Exception {
        require(file.isFile() && file.length()>0 && file.length()<=MAX_ARCHIVE,"IPA fehlt oder ist zu gross");
        String plistName=null; long total=0; int count=0; Set<String> names=new HashSet<>();
        try(ZipFile zip=new ZipFile(file)) {
            Enumeration<? extends ZipEntry> all=zip.entries(); byte[] buffer=new byte[65536];
            while(all.hasMoreElements()) {
                ZipEntry entry=all.nextElement(); String name=entry.getName(); safeName(name);
                require(names.add(name),"Doppelter Archiveintrag");
                require(++count<=30000 && entry.getSize()>=0 && entry.getSize()<=256L*1024*1024,"Archivlimit ueberschritten");
                if(entry.isDirectory()) continue;
                total+=entry.getSize(); require(total<=MAX_CONTENT,"Entpacktes Archiv zu gross");
                CRC32 crc=new CRC32(); long size=0;
                try(InputStream in=zip.getInputStream(entry)) {
                    int n; while((n=in.read(buffer))!=-1) {
                        size+=n; require(size<=entry.getSize(),"Archiveintrag groesser als angegeben");crc.update(buffer,0,n);
                    }
                }
                require(size==entry.getSize() && crc.getValue()==entry.getCrc(),"Beschaedigter Archiveintrag (CRC)");
                if(name.matches("Payload/[^/]+\\.app/Info\\.plist")) {
                    require(plistName==null,"Mehrere Apps in der IPA");plistName=name;
                }
            }
            require(plistName!=null,"Info.plist fehlt");
            Map<?,?> info=plist(read(zip,zip.getEntry(plistName),1024*1024));
            require(BUNDLE.equals(info.get("CFBundleIdentifier")),"Diese IPA ist nicht Hunter Island");
            Object exe=info.get("CFBundleExecutable");
            require(exe instanceof String && ((String)exe).matches("[A-Za-z0-9_. -]{1,128}") &&
                !((String)exe).contains(".."),"Ungueltiger App-Dateiname");
            String prefix=plistName.substring(0,plistName.length()-"Info.plist".length());
            macho(read(zip,zip.getEntry(prefix+exe),64*1024*1024));
            String version=String.valueOf(info.get("CFBundleShortVersionString"));
            require(version.length()<=32 && version.matches("[0-9]+(?:\\.[0-9]+){0,5}"),"Ungueltige Spielversion");
            String minimum=String.valueOf(info.get("MinimumOSVersion"));
            require(minimum.length()<=32 && minimum.matches("[0-9]+(?:\\.[0-9]+){0,3}"),"Ungueltige iOS-Mindestversion");
            return new Info(version,minimum,digest(file));
        }
    }
    private static long u32(byte[] data,int offset,boolean little) throws IOException {
        require(offset>=0 && (long)offset+4<=data.length,"Abgeschnittenes Mach-O");
        long value=0;
        for(int i=0;i<4;i++) value=(value<<8)|(data[offset+(little?3-i:i)]&255);
        return value;
    }
    private static void macho(byte[] bytes) throws IOException {
        long magic=u32(bytes,0,false); int arms=0;
        if(magic==0xcafebabeL || magic==0xbebafecaL) {
            boolean little=magic==0xbebafecaL;long n=u32(bytes,4,little);
            require(n>0 && n<=16 && 8+20*n<=bytes.length,"Ungueltiger Fat-Header");
            for(int i=0;i<n;i++) {
                int p=8+i*20;long cpu=u32(bytes,p,little),offset=u32(bytes,p+8,little),size=u32(bytes,p+12,little);
                require(offset>=8+20*n && size>=28 && offset+size<=bytes.length,"Ungueltige Mach-O-Grenzen");
                if(cpu==12) {thin(bytes,(int)offset,(int)size);arms++;}
            }
        } else {thin(bytes,0,bytes.length);arms++;}
        require(arms>0,"Keine unterstuetzte 32-Bit-ARM-App");
    }
    private static void thin(byte[] bytes,int base,int size) throws IOException {
        long magic=u32(bytes,base,false);boolean little=magic==0xcefaedfeL;
        require(magic==0xcefaedfeL || magic==0xfeedfaceL,"Kein 32-Bit-Mach-O");
        require(u32(bytes,base+4,little)==12,"Keine 32-Bit-ARM-App");
        long n=u32(bytes,base+16,little),commands=u32(bytes,base+20,little);
        require(size>=28 && n<=8192 && 28+commands<=size,"Ungueltige Mach-O-Kommandos");
        int cursor=base+28;long end=(long)cursor+commands;
        for(int i=0;i<n;i++) {
            require(cursor+8<=end,"Abgeschnittenes Mach-O-Kommando");
            long type=u32(bytes,cursor,little),length=u32(bytes,cursor+4,little);
            require(length>=8 && cursor+length<=end,"Ungueltige Mach-O-Kommandolaenge");
            if(type==0x21 || type==0x2c) {
                require(length>=20,"Abgeschnittene Verschluesselungsinfo");
                require(u32(bytes,cursor+16,little)==0,"Die IPA ist noch FairPlay-verschluesselt");
            }
            cursor+=(int)length;
        }
    }
    private static Map<?,?> plist(byte[] data) throws Exception {
        if(data.length>=8 && new String(data,0,8,StandardCharsets.US_ASCII).equals("bplist00"))
            return new BinaryPlist(data).dictionary();
        DocumentBuilderFactory factory=DocumentBuilderFactory.newInstance();
        factory.setFeature("http://xml.org/sax/features/external-general-entities",false);
        factory.setFeature("http://xml.org/sax/features/external-parameter-entities",false);
        factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd",false);
        factory.setXIncludeAware(false);factory.setExpandEntityReferences(false);
        javax.xml.parsers.DocumentBuilder builder=factory.newDocumentBuilder();
        builder.setEntityResolver((publicId,systemId)->new InputSource(new StringReader("")));
        Document doc=builder.parse(new ByteArrayInputStream(data));
        require(doc.getDocumentElement().getTagName().equals("plist"),"Plist-Wurzel fehlt");
        Element dict=null;NodeList roots=doc.getDocumentElement().getChildNodes();
        for(int i=0;i<roots.getLength();i++)if(roots.item(i) instanceof Element){require(dict==null && ((Element)roots.item(i)).getTagName().equals("dict"),"Plist ist kein einzelnes Dictionary");dict=(Element)roots.item(i);}
        require(dict!=null,"Plist-Wurzel fehlt");Map<String,Object> out=new HashMap<>();
        String key=null;NodeList children=dict.getChildNodes();
        for(int i=0;i<children.getLength();i++) if(children.item(i) instanceof Element) {
            Element element=(Element)children.item(i);
            if(element.getTagName().equals("key")){require(key==null,"Plist-Wert fehlt");key=element.getTextContent();}
            else if(key!=null){require(!out.containsKey(key),"Doppelter Plist-Schluessel");out.put(key,element.getTagName().equals("string")?element.getTextContent():null);key=null;}
            else throw invalid("Plist-Schluessel fehlt");
        }
        require(key==null,"Plist-Wert fehlt");
        return out;
    }
    private static final class BinaryPlist {
        final byte[] data;final int referenceSize,objects,top;final long table;final long[] offsets;
        final Set<Integer> reading=new HashSet<>();
        final Object[] decoded;final boolean[] complete;long decodedBytes;int visits;
        BinaryPlist(byte[] data) throws IOException {
            this.data=data;require(data.length>=40,"Plist-Trailer fehlt");int trailer=data.length-32;
            int offsetSize=data[trailer+6]&255;referenceSize=data[trailer+7]&255;
            long count=number(trailer+8,8);long topValue=number(trailer+16,8);table=number(trailer+24,8);
            require(count>0 && count<=65536 && topValue<count && offsetSize>=1 && offsetSize<=8 &&
                referenceSize>=1 && referenceSize<=8 && table>=8 && table+count*offsetSize<=trailer,"Ungueltiger Plist-Trailer");
            objects=(int)count;top=(int)topValue;offsets=new long[objects];decoded=new Object[objects];complete=new boolean[objects];
            for(int i=0;i<objects;i++){offsets[i]=number((int)table+i*offsetSize,offsetSize);require(offsets[i]>=8 && offsets[i]<table,"Plist-Offset ausserhalb der Daten");}
        }
        long number(int offset,int length) throws IOException {
            require(length>=1 && length<=8 && offset>=0 && (long)offset+length<=data.length,"Plist-Zahl ausserhalb der Daten");
            long n=0;for(int i=0;i<length;i++){require(n<=Long.MAX_VALUE>>>8,"Plist-Zahl zu gross");n=(n<<8)|(data[offset+i]&255);}return n;
        }
        Map<?,?> dictionary() throws IOException {Object value=object(top,0);require(value instanceof Map,"Plist ist kein Dictionary");return (Map<?,?>)value;}
        int reference(int offset) throws IOException {
            long ref=number(offset,referenceSize);require(ref<objects,"Plist-Referenz ausserhalb der Objekttabelle");return (int)ref;
        }
        Object object(int ref,int depth) throws IOException {
            require(ref>=0 && ref<objects && depth<=48 && ++visits<=262144,"Plist zu komplex oder Referenz ungueltig");
            if(complete[ref])return decoded[ref];
            require(reading.add(ref),"Zyklische Plist-Referenz");
            try {Object value=decode(ref,depth);decoded[ref]=value;complete[ref]=true;return value;}
            finally{reading.remove(ref);}
        }
        Object decode(int ref,int depth) throws IOException {
                int p=(int)offsets[ref],tag=data[p++]&255,type=tag>>4;long length=tag&15;
                if(type==0)return tag==9?Boolean.TRUE:tag==8?Boolean.FALSE:null;
                if(type==1){require(length<=3,"Plist-Integer zu gross");return number(p,1<<(int)length);}
                if(length==15){int marker=data[p++]&255;require((marker>>4)==1 && (marker&15)<=3,"Ungueltige Plist-Laenge");int width=1<<(marker&15);length=number(p,width);p+=width;}
                require(length<=1024*1024,"Plist-Objekt zu gross");
                long bytes=type==6?length*2:(type==10?length*referenceSize:(type==13?length*referenceSize*2:length));
                require(p+bytes<=table,"Plist-Objekt ausserhalb der Daten");
                decodedBytes+=bytes;require(decodedBytes<=8*1024*1024,"Plist-Dekodierbudget ueberschritten");
                if(type==5 || type==6)return new String(data,p,(int)bytes,type==5?StandardCharsets.US_ASCII:StandardCharsets.UTF_16BE);
                if(type==10){List<Object> list=new ArrayList<>();for(int i=0;i<length;i++)list.add(object(reference(p+i*referenceSize),depth+1));return list;}
                if(type==13){Map<Object,Object> map=new HashMap<>();for(int i=0;i<length;i++) {
                    Object key=object(reference(p+i*referenceSize),depth+1);
                    require(key instanceof String && !map.containsKey(key),"Ungueltiger oder doppelter Plist-Schluessel");
                    map.put(key,object(reference(p+((int)length+i)*referenceSize),depth+1));
                }return map;}
                return null; // Non-metadata values (data/date/real) are not used.
        }
    }
}
