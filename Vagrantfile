Vagrant.configure("2") do |config|
  config.vm.box = "ubuntu/xenial64"
  config.vm.provision "shell",
    inline: "sudo apt-get -q update && sudo apt-get install -yq python"
  config.vm.provision "ansible" do |ansible|
    ansible.playbook = "playbook.yml"
  end

end
