# Author: D.A.Pelasgus
 
let
   # Unstable Channel | Rolling Release
   pkgs = import (fetchTarball("channel:nixpkgs-unstable")) { };
   packages = with pkgs; [
     pkg-config
     webkitgtk_4_1
     libayatana-appindicator
     libappindicator-gtk3
   ];
 in
 pkgs.mkShell {
   buildInputs = packages;
 }
