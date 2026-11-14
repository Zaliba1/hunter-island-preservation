# SPDX-License-Identifier: MPL-2.0
"""Create your own local signing material outside the distributed source tree."""
from pathlib import Path
import argparse,json,os,secrets,subprocess

def create(java_home,directory):
    root=Path(__file__).resolve().parents[1];directory=Path(directory).resolve()
    if directory.is_relative_to(root):raise ValueError('Choose a private directory outside the source checkout')
    directory.mkdir(parents=True,exist_ok=True)
    key=directory/'hunter-preview.p12';config=directory/'local-preview-signing.json'
    if key.exists() or config.exists():raise ValueError('Refuse to overwrite an existing signing identity')
    password=secrets.token_urlsafe(32);env=dict(os.environ,HI_LOCAL_KEY_PASSWORD=password)
    tool=Path(java_home)/'bin'/('keytool.exe' if os.name=='nt' else 'keytool')
    subprocess.run([str(tool),'-genkeypair','-alias','hunter-preview','-keyalg','RSA','-keysize','3072',
        '-validity','10000','-storetype','PKCS12','-keystore',str(key),'-storepass:env','HI_LOCAL_KEY_PASSWORD',
        '-keypass:env','HI_LOCAL_KEY_PASSWORD','-dname','CN=Hunter Island Preview,O=Independent Preservation','-noprompt'],env=env,check=True)
    with config.open('x',encoding='utf-8') as out:json.dump(dict(password=password),out)
    print('Private signing identity created. Keep both files private and retain them for your future updates.')

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--java-home',type=Path,required=True);p.add_argument('--directory',type=Path,required=True);a=p.parse_args();create(a.java_home,a.directory)
